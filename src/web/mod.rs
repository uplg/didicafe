pub mod error;
pub mod extractors;
pub mod portal;
pub mod admin;
pub mod api;
pub mod captive;
pub mod cpd;

pub use error::AppError;

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;
use axum::{Router, response::Redirect, routing::any};
use axum::middleware::{self, Next};
use axum::http::HeaderValue;
use axum::response::{IntoResponse, Response};
use axum::extract::{ConnectInfo, Request, State};
use tower_http::services::ServeDir;

use crate::AppState;
use crate::config::ip_in_cidr;

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
            "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self'; font-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'"
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

/// `Captive-Portal:` HTTP header middleware (RFC 8908 §4).
///
/// Added to all responses on the portal-facing listener so any HTTP
/// interaction by a captive client carries a pointer to the CAPPORT API.
/// Modern OS captive portal browsers honor this header and use the URL
/// to query session state without scraping HTML.
async fn captive_portal_header(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let mut response = next.run(request).await;
    if let Some(value) = captive::captive_portal_header_value(&state.config) {
        response.headers_mut().insert("Captive-Portal", value);
    }
    response
}

/// HSTS header middleware for the admin HTTPS listener.
///
/// Tells browsers to always use HTTPS for this host:port combination.
/// `max-age=63072000` = 2 years. No `includeSubDomains` since this is a
/// single local domain.
async fn hsts_header(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        "Strict-Transport-Security",
        HeaderValue::from_static("max-age=63072000"),
    );
    response
}

/// Admin IP allowlist middleware.
///
/// If `admin.allowed_networks` is configured (non-empty), only IPs within
/// those CIDR ranges can access the admin router. Returns 403 otherwise.
/// If the list is empty, all IPs are accepted (default).
async fn admin_network_filter(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    let allowed = &state.config.admin.allowed_networks;

    if !allowed.is_empty() {
        let client_ip = addr.ip();
        let is_allowed = allowed.iter().any(|cidr| ip_in_cidr(client_ip, cidr));
        if !is_allowed {
            tracing::warn!(%client_ip, "admin access denied: IP not in allowed_networks");
            return (
                axum::http::StatusCode::FORBIDDEN,
                "access denied",
            ).into_response();
        }
    }

    next.run(request).await
}

/// Build the portal-only router (HTTP listener).
///
/// Contains public captive portal routes, CPD probes, static files, and
/// a catch-all redirect to `/portal`. Does NOT include `/admin/*` or `/api/*`.
pub fn portal_router(state: Arc<AppState>) -> Router {
    let static_dir = resolve_static_dir();

    Router::new()
        // Public captive portal routes
        .merge(portal::routes())
        // CPD (Captive Portal Detection) probe handlers
        .merge(cpd::routes())
        // RFC 8908 Captive Portal API (JSON)
        .merge(captive::routes())
        // Static files (CSS, JS, favicon, etc.)
        .nest_service("/static", ServeDir::new(static_dir))
        // Catch-all: any unmatched route redirects to the portal.
        // This handles CPD probes from less common OSes and any
        // stray HTTP requests DNATed by nftables.
        .fallback(any(|| async { Redirect::to("/portal") }))
        // Captive-Portal header (RFC 8908 §4) on all portal-side responses
        .layer(middleware::from_fn_with_state(Arc::clone(&state), captive_portal_header))
        // Security headers on all responses
        .layer(middleware::from_fn(security_headers))
        .with_state(state)
}

/// Build the admin-only router (HTTPS listener when TLS is enabled).
///
/// Contains admin UI routes, REST API routes, and static files.
/// Does NOT include portal routes or CPD probes.
/// Includes HSTS header and optional IP allowlist middleware.
pub fn admin_router(state: Arc<AppState>) -> Router {
    let static_dir = resolve_static_dir();

    Router::new()
        // Admin UI routes
        .merge(admin::routes())
        // REST API routes
        .merge(api::routes())
        // Static files (CSS, JS needed by admin pages)
        .nest_service("/static", ServeDir::new(static_dir))
        // Catch-all: redirect to admin login
        .fallback(any(|| async { Redirect::to("/admin/login") }))
        // Admin IP allowlist (runs before request processing)
        .layer(middleware::from_fn_with_state(Arc::clone(&state), admin_network_filter))
        // HSTS header (only on the HTTPS listener)
        .layer(middleware::from_fn(hsts_header))
        // Security headers on all responses
        .layer(middleware::from_fn(security_headers))
        .with_state(state)
}

/// Build a combined router serving both portal and admin (dev mode / TLS disabled).
///
/// This is the current behavior: all routes on a single listener.
/// Used when `tls.enabled = false` in the config.
pub fn combined_router(state: Arc<AppState>) -> Router {
    let static_dir = resolve_static_dir();

    Router::new()
        // Public captive portal routes
        .merge(portal::routes())
        // CPD (Captive Portal Detection) probe handlers
        .merge(cpd::routes())
        // RFC 8908 Captive Portal API (JSON)
        .merge(captive::routes())
        // Admin UI routes
        .merge(admin::routes())
        // REST API routes
        .merge(api::routes())
        // Static files (CSS, JS, favicon, etc.)
        .nest_service("/static", ServeDir::new(static_dir))
        // Catch-all: any unmatched route redirects to the portal.
        .fallback(any(|| async { Redirect::to("/portal") }))
        // Captive-Portal header (RFC 8908 §4)
        .layer(middleware::from_fn_with_state(Arc::clone(&state), captive_portal_header))
        // Security headers on all responses
        .layer(middleware::from_fn(security_headers))
        .with_state(state)
}

/// Resolve the `static/` directory to an absolute path.
///
/// Strategy: use the executable's directory as the base. On the embedded
/// target, the binary lives in `/opt/didicafe/didicafe` and static files
/// in `/opt/didicafe/static/`. In development, `cargo run` sets the CWD
/// to the project root, so `./static/` works too.
fn resolve_static_dir() -> std::path::PathBuf {
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        let candidate = parent.join("static");
        if candidate.is_dir() {
            return candidate;
        }
    }
    // Fallback: relative path (works when CWD is project root)
    std::path::PathBuf::from("static")
}

#[cfg(test)]
mod tests {
    use tower::ServiceExt;

    use crate::test_utils::{test_get, test_state};

    /// Every portal-side response carries the `Captive-Portal:` header
    /// (RFC 8908 §4) pointing to the CAPPORT API URL.
    #[tokio::test]
    async fn portal_router_emits_captive_portal_header() {
        let state = test_state().await;
        let router = super::portal_router(state);

        let response = router.oneshot(test_get("/portal")).await.unwrap();
        let header = response
            .headers()
            .get("Captive-Portal")
            .expect("Captive-Portal header must be set")
            .to_str()
            .unwrap();

        assert!(header.starts_with('<'), "header must start with '<': {header}");
        assert!(header.ends_with('>'), "header must end with '>': {header}");
        assert!(header.contains("/api/captive"), "header must reference API URL: {header}");
    }

    /// Same for the combined dev router.
    #[tokio::test]
    async fn combined_router_emits_captive_portal_header() {
        let state = test_state().await;
        let router = super::combined_router(state);

        let response = router.oneshot(test_get("/api/captive")).await.unwrap();
        let header = response
            .headers()
            .get("Captive-Portal")
            .expect("Captive-Portal header must be set")
            .to_str()
            .unwrap();
        assert!(header.contains("/api/captive"));
    }

    /// Admin router does NOT emit the Captive-Portal header — it's a portal-side
    /// concern only and admins are not captive clients.
    #[tokio::test]
    async fn admin_router_does_not_emit_captive_portal_header() {
        let state = test_state().await;
        let router = super::admin_router(state);

        let response = router.oneshot(test_get("/admin/login")).await.unwrap();
        assert!(
            response.headers().get("Captive-Portal").is_none(),
            "admin router must not emit Captive-Portal header"
        );
    }
}
