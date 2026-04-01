//! Captive Portal Detection (CPD) probe handlers.
//!
//! Modern OSes probe specific HTTP URLs to detect captive portals:
//! - Apple (iOS/macOS): `GET /hotspot-detect.html`
//! - Android: `GET /generate_204`
//! - Windows: `GET /connecttest.txt`
//! - Linux (NetworkManager/GNOME): `GET /check_network_status.txt`
//!
//! nftables DNATs all HTTP (port 80) traffic from unauthenticated clients
//! to our server. We respond with a redirect to the portal page, which
//! triggers the OS to open its built-in captive portal browser.
//!
//! Redirections use the configured domain (e.g. `http://didicafe.local/portal`)
//! so the user sees a friendly URL instead of a raw IP address.

use std::sync::Arc;
use axum::{Router, extract::State, response::Redirect, routing::get};

use crate::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        // Apple CNA (Captive Network Assistant)
        .route("/hotspot-detect.html", get(redirect_to_portal))
        // Android connectivity check
        .route("/generate_204", get(redirect_to_portal))
        // Windows NCSI (Network Connectivity Status Indicator)
        .route("/connecttest.txt", get(redirect_to_portal))
        // Linux NetworkManager / GNOME
        .route("/check_network_status.txt", get(redirect_to_portal))
}

/// All CPD probes redirect to the portal page via 302.
///
/// 302 (not 301) because we don't want the OS to cache the redirect
/// permanently -- after authentication, the probes should succeed normally
/// (though they'll go through the real internet, not our server).
///
/// Uses the configured domain so the captive browser shows a friendly URL
/// (e.g. `http://didicafe.local/portal`) instead of a raw IP.
async fn redirect_to_portal(State(state): State<Arc<AppState>>) -> Redirect {
    let domain = &state.config.portal.domain;
    let port = state.config.server.port;

    // If the portal runs on the standard HTTP port (80), omit the port
    // from the URL. Otherwise include it (e.g. `:8080`).
    let url = if port == 80 {
        format!("http://{domain}/portal")
    } else {
        format!("http://{domain}:{port}/portal")
    };

    Redirect::to(&url)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use axum::http::StatusCode;
    use tower::ServiceExt;

    use crate::test_utils::{test_state, test_get};

    fn cpd_router(state: Arc<crate::AppState>) -> axum::Router {
        super::routes().with_state(state)
    }

    #[tokio::test]
    async fn test_cpd_apple_redirects_to_domain() {
        let state = test_state().await;
        let app = cpd_router(state);
        let response = app.oneshot(test_get("/hotspot-detect.html")).await.unwrap();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        let location = response.headers().get("location").unwrap().to_str().unwrap();
        assert_eq!(location, "http://didicafe.local:8080/portal");
    }

    #[tokio::test]
    async fn test_cpd_android_redirects_to_domain() {
        let state = test_state().await;
        let app = cpd_router(state);
        let response = app.oneshot(test_get("/generate_204")).await.unwrap();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        let location = response.headers().get("location").unwrap().to_str().unwrap();
        assert!(location.contains("didicafe.local"));
        assert!(location.ends_with("/portal"));
    }

    #[tokio::test]
    async fn test_cpd_windows_redirects_to_domain() {
        let state = test_state().await;
        let app = cpd_router(state);
        let response = app.oneshot(test_get("/connecttest.txt")).await.unwrap();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        let location = response.headers().get("location").unwrap().to_str().unwrap();
        assert!(location.contains("/portal"));
    }

    #[tokio::test]
    async fn test_cpd_linux_redirects_to_domain() {
        let state = test_state().await;
        let app = cpd_router(state);
        let response = app.oneshot(test_get("/check_network_status.txt")).await.unwrap();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        let location = response.headers().get("location").unwrap().to_str().unwrap();
        assert!(location.contains("/portal"));
    }

    #[tokio::test]
    async fn test_cpd_port_80_omits_port() {
        let state = test_state().await;
        // Modify config to use port 80
        let mut config = state.config.clone();
        config.server.port = 80;

        // Create new state with port 80
        let db = crate::db::Database::open(":memory:").await.unwrap();
        db.migrate().await.unwrap();
        let state = Arc::new(crate::AppState {
            db,
            config,
            firewall: Arc::new(crate::firewall::MockFirewall::new()),
            admin_sessions: crate::services::admin_session::AdminSessionStore::new(3600),
            rate_limiter: crate::services::rate_limit::RateLimiter::new(&state.config.rate_limit),
            admin_rate_limiter: crate::services::rate_limit::RateLimiter::new(&state.config.rate_limit),
            portal_csrf_store: crate::services::csrf::PortalCsrfStore::new(),
            login_csrf_store: crate::services::csrf::PortalCsrfStore::new(),
        });

        let app = cpd_router(state);
        let response = app.oneshot(test_get("/hotspot-detect.html")).await.unwrap();

        let location = response.headers().get("location").unwrap().to_str().unwrap();
        assert_eq!(location, "http://didicafe.local/portal");
    }
}
