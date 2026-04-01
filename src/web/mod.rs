pub mod error;
pub mod extractors;
pub mod portal;
pub mod admin;
pub mod api;
pub mod cpd;

pub use error::AppError;

use std::collections::BTreeMap;
use std::sync::Arc;
use axum::{Router, response::Redirect, routing::any};
use axum::middleware::{self, Next};
use axum::http::HeaderValue;
use axum::response::Response;
use axum::extract::Request;
use tower_http::services::ServeDir;

use crate::AppState;

/// An i18n message with a key and optional named interpolation arguments.
///
/// Used by templates to render `data-i18n` and `data-i18n-args` attributes.
/// The frontend's `i18n.js` reads these attributes and resolves the translated
/// string with `{{placeholder}}` replacement.
///
/// Example:
///   key = "admin.manage.tokens_generated", args = { "count": "5" }
///   Template renders: <div data-i18n="admin.manage.tokens_generated" data-i18n-args='{"count":"5"}'>
///   Frontend resolves: "5 code(s) naorina" (in Malagasy)
#[derive(Debug, Clone)]
pub struct I18nMessage {
    /// Translation key (e.g. "admin.manage.plan_created")
    pub key: String,
    /// Named interpolation arguments (e.g. {"name": "WiFi 1h", "count": "5"}).
    /// Uses BTreeMap for deterministic JSON output in templates.
    pub args: BTreeMap<String, String>,
}

impl I18nMessage {
    /// Create a message with interpolation arguments from key-value pairs.
    pub fn with_args(key: impl Into<String>, args: impl IntoIterator<Item = (&'static str, String)>) -> Self {
        Self {
            key: key.into(),
            args: args.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
        }
    }

    /// Render the `data-i18n-args` attribute value as a JSON string.
    /// Returns an empty string if there are no args (template can skip the attribute).
    pub fn args_json(&self) -> String {
        if self.args.is_empty() {
            return String::new();
        }
        // Manual JSON construction to avoid serde dependency for this small case.
        // BTreeMap iteration order is deterministic (sorted by key).
        let pairs: Vec<String> = self.args.iter()
            .map(|(k, v)| {
                // Escape double quotes and backslashes in values for JSON safety
                let escaped = v.replace('\\', "\\\\").replace('"', "\\\"");
                format!("\"{}\":\"{}\"", k, escaped)
            })
            .collect();
        format!("{{{}}}", pairs.join(","))
    }
}

/// Security headers middleware (OWASP best practices).
///
/// Applied to all responses. These headers mitigate:
/// - `X-Content-Type-Options: nosniff` — MIME sniffing attacks
/// - `X-Frame-Options: DENY` — clickjacking
/// - `Content-Security-Policy` — XSS, injection attacks
/// - `Referrer-Policy: strict-origin-when-cross-origin` — information leakage
/// - `X-XSS-Protection: 0` — disable legacy XSS auditor (CSP is the modern defense)
/// - `Permissions-Policy` — restrict access to device APIs
/// - `Cache-Control: no-store` on admin/api paths — prevent caching of sensitive data
async fn security_headers(request: Request, next: Next) -> Response {
    let is_sensitive = request.uri().path().starts_with("/admin")
        || request.uri().path().starts_with("/api");
    let is_static = request.uri().path().starts_with("/static");

    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    headers.insert(
        "X-Content-Type-Options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        "X-Frame-Options",
        HeaderValue::from_static("DENY"),
    );
    headers.insert(
        "Content-Security-Policy",
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self'; font-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'"
        ),
    );
    headers.insert(
        "Referrer-Policy",
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        "X-XSS-Protection",
        HeaderValue::from_static("0"),
    );
    headers.insert(
        "Permissions-Policy",
        HeaderValue::from_static("camera=(), microphone=(), geolocation=(), payment=(), usb=()"),
    );

    // Prevent caching of admin/API responses (session data, tokens, audit log)
    if is_sensitive {
        headers.insert(
            "Cache-Control",
            HeaderValue::from_static("no-store, no-cache, must-revalidate"),
        );
        headers.insert(
            "Pragma",
            HeaderValue::from_static("no-cache"),
        );
    } else if is_static {
        // Cache static assets (CSS, JS, images) for 1 hour.
        // These rarely change and caching reduces load on the embedded router.
        headers.insert(
            "Cache-Control",
            HeaderValue::from_static("public, max-age=3600"),
        );
    }

    response
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        // Public captive portal routes
        .merge(portal::routes())
        // CPD (Captive Portal Detection) probe handlers
        .merge(cpd::routes())
        // Admin UI routes
        .merge(admin::routes())
        // REST API routes
        .merge(api::routes())
        // Static files (CSS, favicon, etc.)
        .nest_service("/static", ServeDir::new("static"))
        // Catch-all: any unmatched route redirects to the portal.
        // This handles CPD probes from less common OSes and any
        // stray HTTP requests DNATed by nftables.
        .fallback(any(|| async { Redirect::to("/portal") }))
        // Security headers on all responses
        .layer(middleware::from_fn(security_headers))
        .with_state(state)
}
