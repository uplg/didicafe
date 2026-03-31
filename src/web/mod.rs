pub mod error;
pub mod extractors;
pub mod portal;
pub mod admin;
pub mod api;
pub mod cpd;

pub use error::AppError;

use std::sync::Arc;
use axum::{Router, response::Redirect, routing::any};
use axum::middleware::{self, Next};
use axum::http::HeaderValue;
use axum::response::Response;
use axum::extract::Request;
use tower_http::services::ServeDir;

use crate::AppState;

/// Security headers middleware (OWASP best practices).
///
/// Applied to all responses. These headers mitigate:
/// - `X-Content-Type-Options: nosniff` — MIME sniffing attacks
/// - `X-Frame-Options: DENY` — clickjacking
/// - `Content-Security-Policy` — XSS, injection attacks
/// - `Referrer-Policy: strict-origin-when-cross-origin` — information leakage
/// - `X-XSS-Protection: 0` — disable legacy XSS auditor (CSP is the modern defense)
async fn security_headers(request: Request, next: Next) -> Response {
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
