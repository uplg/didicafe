//! RFC 8908: Captive Portal API.
//!
//! Modern clients (iOS 14+, macOS 11+, Windows 11) discover this endpoint
//! via DHCP option 114 (RFC 8910) and query it to learn whether they are
//! captive and how much session time remains, without scraping HTML.
//!
//! Response payload (RFC 8908 §5):
//!   {
//!     "captive": true|false,
//!     "user-portal-url": "<portal>",      // required when captive=true
//!     "venue-info-url": "<plans page>",   // optional
//!     "can-extend-session": false,        // we use one-shot tokens
//!     "seconds-remaining": <int>          // when captive=false
//!   }
//!
//! Content type: `application/captive+json` (RFC 8908 §6).

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    extract::{ConnectInfo, State},
    http::{HeaderValue, StatusCode, header::CONTENT_TYPE},
    response::Response,
    routing::get,
};
use serde::Serialize;

use super::error::AppError;
use crate::AppState;
use crate::config::Config;
use crate::net::arp;
use crate::net::mac::is_valid_mac;

/// Content type defined in RFC 8908 §6.
pub const CAPPORT_CONTENT_TYPE: &str = "application/captive+json";

/// CAPPORT API JSON body (RFC 8908 §5).
///
/// Field names use kebab-case as mandated by the RFC. Optional fields are
/// omitted from the output when `None`.
#[derive(Serialize)]
struct CapportResponse<'a> {
    /// True when the client must complete portal authentication.
    captive: bool,
    /// User-facing portal page. Required when `captive=true` (we always include it).
    #[serde(rename = "user-portal-url")]
    user_portal_url: &'a str,
    /// Page with venue info (plans, contact, opening hours).
    #[serde(rename = "venue-info-url")]
    venue_info_url: &'a str,
    /// Whether the user can extend their session by re-visiting the portal.
    /// We use one-shot tokens, so this is always false.
    #[serde(rename = "can-extend-session")]
    can_extend_session: bool,
    /// Seconds remaining in the active session. Only set when `captive=false`.
    #[serde(rename = "seconds-remaining", skip_serializing_if = "Option::is_none")]
    seconds_remaining: Option<i64>,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/captive", get(captive_api))
}

/// GET /api/captive — RFC 8908 endpoint.
///
/// Identifies the client by ARP-resolved MAC. If the MAC has an active
/// session, returns `captive: false` with `seconds-remaining`. Otherwise
/// returns `captive: true` with the portal URL.
///
/// Returns plain `Response` (not `Json<...>`) because the Content-Type
/// must be `application/captive+json`, not `application/json`.
async fn captive_api(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Result<Response, AppError> {
    let mac = resolve_client_mac(addr.ip()).await;

    let session = match mac {
        Some(ref m) => state
            .db
            .get_session_by_mac(m)
            .await
            .map_err(AppError::Internal)?,
        None => None,
    };

    let portal_url = portal_page_url(&state.config);
    let venue_url = venue_info_url(&state.config);

    let body = match session {
        Some(s) if s.remaining_seconds() > 0 => CapportResponse {
            captive: false,
            user_portal_url: &portal_url,
            venue_info_url: &venue_url,
            can_extend_session: false,
            seconds_remaining: Some(s.remaining_seconds()),
        },
        _ => CapportResponse {
            captive: true,
            user_portal_url: &portal_url,
            venue_info_url: &venue_url,
            can_extend_session: false,
            seconds_remaining: None,
        },
    };

    let json = serde_json::to_string(&body)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("CAPPORT serialize: {e}")))?;

    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, CAPPORT_CONTENT_TYPE)
        .body(Body::from(json))
        .map_err(|e| AppError::Internal(anyhow::anyhow!("CAPPORT response build: {e}")))
}

/// Resolve the client's MAC from their IP via the ARP table.
///
/// Unlike the `ClientInfo` extractor used by HTML routes, the CAPPORT API
/// is meant to *report* connectivity state — including the case where we
/// can't identify the client. On lookup failure we return `None`, and the
/// handler responds with `captive: true` (safe default).
async fn resolve_client_mac(ip: IpAddr) -> Option<String> {
    #[cfg(test)]
    if ip.is_loopback() {
        return Some("02:00:00:00:00:01".to_string());
    }
    arp::lookup_mac(ip).await.filter(|m| is_valid_mac(m))
}

/// Build the absolute URL to the user-facing portal page.
///
/// Uses the configured domain (e.g. `wifi.didicafe`) so devices see a
/// friendly URL. Omits the port when running on the standard HTTP port (80).
pub fn portal_page_url(config: &Config) -> String {
    build_portal_url(config, "/portal")
}

/// Build the absolute URL to the venue info page (plans + contact).
pub fn venue_info_url(config: &Config) -> String {
    build_portal_url(config, "/portal/plans")
}

/// Build the absolute URL to the CAPPORT API endpoint itself.
///
/// Used by the `Captive-Portal` HTTP header middleware and by dnsmasq
/// option 114 (operator copies this value into config).
pub fn capport_api_url(config: &Config) -> String {
    build_portal_url(config, "/api/captive")
}

fn build_portal_url(config: &Config, path: &str) -> String {
    let domain = &config.portal.domain;
    let port = config.server.port;
    if port == 80 {
        format!("http://{domain}{path}")
    } else {
        format!("http://{domain}:{port}{path}")
    }
}

/// Build the `Captive-Portal` HTTP header value (RFC 8908 §4).
///
/// Format: `<api-url>` (URI-reference wrapped in angle brackets).
pub fn captive_portal_header_value(config: &Config) -> Option<HeaderValue> {
    let url = capport_api_url(config);
    HeaderValue::from_str(&format!("<{url}>")).ok()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::http::StatusCode;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use crate::test_utils::{test_get, test_state};

    fn captive_router(state: Arc<crate::AppState>) -> axum::Router {
        super::routes().with_state(state)
    }

    #[tokio::test]
    async fn unauthenticated_returns_captive_true() {
        let state = test_state().await;
        let app = captive_router(state);

        let response = app.oneshot(test_get("/api/captive")).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["captive"], true);
        assert_eq!(json["can-extend-session"], false);
        assert_eq!(json["user-portal-url"], "http://wifi.didicafe:8080/portal");
        assert_eq!(
            json["venue-info-url"],
            "http://wifi.didicafe:8080/portal/plans"
        );
        // seconds-remaining must be omitted when captive=true
        assert!(json.get("seconds-remaining").is_none());
    }

    #[tokio::test]
    async fn content_type_is_capport_json() {
        let state = test_state().await;
        let app = captive_router(state);

        let response = app.oneshot(test_get("/api/captive")).await.unwrap();
        let ctype = response
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(ctype, "application/captive+json");
    }

    #[tokio::test]
    async fn authenticated_returns_captive_false_with_remaining() {
        let state = test_state().await;

        // Create plan + token + session for the test client MAC
        let plan_id = state.db.create_plan("1h WiFi", 60, 1000).await.unwrap();
        let token_id = state
            .db
            .create_token("DIDI-ABCD-EF23", None, plan_id)
            .await
            .unwrap();
        crate::services::session::create_session(
            &state,
            token_id,
            60,
            "02:00:00:00:00:01",
            "127.0.0.1",
        )
        .await
        .unwrap();

        let app = captive_router(Arc::clone(&state));
        let response = app.oneshot(test_get("/api/captive")).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["captive"], false);
        let remaining = json["seconds-remaining"].as_i64().unwrap();
        assert!(remaining > 0, "expected remaining > 0, got {remaining}");
        assert!(
            remaining <= 3600,
            "expected remaining ≤ 3600, got {remaining}"
        );
        // Portal URL is present even when authenticated (clients can revisit)
        assert!(
            json["user-portal-url"]
                .as_str()
                .unwrap()
                .ends_with("/portal")
        );
    }

    #[tokio::test]
    async fn expired_session_returns_captive_true() {
        let state = test_state().await;

        // Create a session then mark it expired in DB
        let plan_id = state.db.create_plan("1h WiFi", 60, 1000).await.unwrap();
        let token_id = state
            .db
            .create_token("DIDI-EXPD-TK23", None, plan_id)
            .await
            .unwrap();
        let session_id = crate::services::session::create_session(
            &state,
            token_id,
            60,
            "02:00:00:00:00:01",
            "127.0.0.1",
        )
        .await
        .unwrap();
        state.db.expire_session(session_id).await.unwrap();

        let app = captive_router(Arc::clone(&state));
        let response = app.oneshot(test_get("/api/captive")).await.unwrap();

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["captive"], true);
        assert!(json.get("seconds-remaining").is_none());
    }

    #[tokio::test]
    async fn url_omits_default_http_port() {
        // Rebuild state with port 80 (mirrors cpd::tests pattern).
        let base = test_state().await;
        let mut config = base.config.clone();
        config.server.port = 80;

        let db = crate::db::Database::open(":memory:").await.unwrap();
        db.migrate().await.unwrap();
        let state = Arc::new(crate::AppState {
            db,
            config,
            firewall: Arc::new(crate::firewall::MockFirewall::new()),
            admin_sessions: crate::services::admin_session::AdminSessionStore::new(3600),
            rate_limiter: crate::services::rate_limit::RateLimiter::new(&base.config.rate_limit),
            admin_rate_limiter: crate::services::rate_limit::RateLimiter::new(
                &base.config.rate_limit,
            ),
            portal_csrf_store: crate::services::csrf::PortalCsrfStore::new(),
            login_csrf_store: crate::services::csrf::PortalCsrfStore::new(),
        });

        let app = captive_router(state);
        let response = app.oneshot(test_get("/api/captive")).await.unwrap();

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["user-portal-url"], "http://wifi.didicafe/portal");
        assert_eq!(json["venue-info-url"], "http://wifi.didicafe/portal/plans");
    }

    #[test]
    fn captive_portal_header_uses_angle_brackets() {
        use crate::test_utils::test_config;
        let cfg = test_config();
        let value = super::captive_portal_header_value(&cfg).unwrap();
        let s = value.to_str().unwrap();
        assert!(s.starts_with('<') && s.ends_with('>'), "got: {s}");
        assert!(s.contains("/api/captive"));
    }

    #[test]
    fn capport_api_url_uses_configured_domain() {
        use crate::test_utils::test_config;
        let mut cfg = test_config();
        cfg.portal.domain = "wifi.example.org".to_string();
        cfg.server.port = 8080;
        assert_eq!(
            super::capport_api_url(&cfg),
            "http://wifi.example.org:8080/api/captive"
        );
        cfg.server.port = 80;
        assert_eq!(
            super::capport_api_url(&cfg),
            "http://wifi.example.org/api/captive"
        );
    }
}
