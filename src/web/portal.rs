use std::sync::Arc;
use axum::{
    Form, Json,
    Router,
    extract::State,
    response::{IntoResponse, Redirect},
    routing::{get, post},
};
use askama::Template;
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::db::Plan;
use crate::services;
use crate::services::rate_limit::RateLimitResult;
use crate::services::token::TokenLookup;
use super::error::{AppError, render};
use super::extractors::ClientInfo;

// -- Custom Askama filters --

/// Format an integer as a thousands-grouped Ariary string.
/// Uses narrow no-break space (U+202F) as the grouping separator (French style).
/// e.g. 10000 → "10 000", 500 → "500"
fn format_ariary(s: &str) -> String {
    // Only format if purely digits (possibly with leading minus)
    let (sign, digits) = if let Some(rest) = s.strip_prefix('-') {
        ("-", rest)
    } else {
        ("", s)
    };
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return s.to_string();
    }
    let mut result = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            result.push('\u{202f}'); // narrow no-break space
        }
        result.push(ch);
    }
    format!("{sign}{result}")
}

mod filters {
    #[askama::filter_fn]
    pub fn fmt_ariary(
        value: impl std::fmt::Display,
        _env: &dyn askama::Values,
    ) -> askama::Result<String> {
        Ok(super::format_ariary(&value.to_string()))
    }
}

// -- Templates --

#[derive(Template)]
#[template(path = "portal.html")]
struct PortalTemplate {
    error: Option<String>,
    csrf_token: String,
}

#[derive(Template)]
#[template(path = "success.html")]
struct SuccessTemplate {
    remaining_seconds: i64,
}

#[derive(Template)]
#[template(path = "expired.html")]
struct ExpiredTemplate;

#[derive(Template)]
#[template(path = "privacy.html")]
struct PrivacyTemplate;

#[derive(Template)]
#[template(path = "plans.html")]
struct PlansTemplate {
    plans: Vec<Plan>,
    cafe_name: String,
    contact_phone: String,
    contact_name: String,
    contact_hours: String,
}

// -- Form --

#[derive(Deserialize)]
pub struct AuthForm {
    token: String,
    csrf_token: String,
}

// -- Routes --

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/health", get(health_check))
        .route("/portal", get(portal_page))
        .route("/portal/auth", post(portal_auth))
        .route("/portal/success", get(success_page))
        .route("/portal/expired", get(expired_page))
        .route("/portal/status", get(portal_status))
        .route("/portal/privacy", get(privacy_page))
        .route("/portal/plans", get(plans_page))
}

/// GET /health -- health check for process supervision
async fn health_check(State(state): State<Arc<AppState>>) -> axum::http::StatusCode {
    match state.db.get_daily_stats().await {
        Ok(_) => axum::http::StatusCode::OK,
        Err(_) => axum::http::StatusCode::SERVICE_UNAVAILABLE,
    }
}

/// GET /portal -- splash page with token input
async fn portal_page(
    State(state): State<Arc<AppState>>,
    client: ClientInfo,
) -> Result<impl IntoResponse, AppError> {
    let csrf_token = state.portal_csrf_store.generate(&client.mac);
    render(&PortalTemplate { error: None, csrf_token })
}

/// POST /portal/auth -- validate token, create session
async fn portal_auth(
    State(state): State<Arc<AppState>>,
    client: ClientInfo,
    Form(form): Form<AuthForm>,
) -> Result<impl IntoResponse, AppError> {
    // CSRF validation: Synchronizer Token Pattern (server-side)
    if !state.portal_csrf_store.validate(&client.mac, &form.csrf_token) {
        return Ok(render(&PortalTemplate {
            error: Some("portal.error.csrf".to_string()),
            csrf_token: state.portal_csrf_store.generate(&client.mac),
        })?.into_response());
    }

    // Rate limit check BEFORE any DB lookup (OWASP: never leak token existence)
    match state.rate_limiter.check_and_record(client.ip) {
        RateLimitResult::Allowed => {}
        RateLimitResult::Throttled => {
            return Err(AppError::RateLimited { retry_after: None });
        }
        RateLimitResult::Banned { retry_after_seconds } => {
            return Err(AppError::RateLimited {
                retry_after: Some(retry_after_seconds),
            });
        }
    }

    let code = form.token.trim().to_uppercase();

    // Validate token format before any DB lookup (defense-in-depth)
    if let Err(_msg) = services::token::validate_token_format(&state.config.token, &code) {
        return Ok(render(&PortalTemplate {
            error: Some("portal.error.invalid".to_string()),
            csrf_token: state.portal_csrf_store.generate(&client.mac),
        })?.into_response());
    }

    // Validate token against DB
    let lookup = services::token::validate_token(&state.db, &code).await
        .map_err(AppError::Internal)?;

    match lookup {
        TokenLookup::Unused(token) => {
            // Fresh token — create a new session
            match services::session::create_session(
                &state,
                token.id,
                token.duration_minutes,
                &client.mac,
                &client.ip.to_string(),
            ).await {
                Ok(_) => Ok(Redirect::to("/portal/success").into_response()),
                Err(e) => {
                    tracing::error!("session creation failed: {e}");
                    Ok(render(&PortalTemplate {
                        error: Some("portal.error.internal".to_string()),
                        csrf_token: state.portal_csrf_store.generate(&client.mac),
                    })?.into_response())
                }
            }
        }
        TokenLookup::Active(token) => {
            // Token already active — migrate session to new MAC
            // (handles MAC randomization after WiFi reconnect)
            match services::session::migrate_session(
                &state,
                token.id,
                &client.mac,
                &client.ip.to_string(),
            ).await {
                Ok(_) => Ok(Redirect::to("/portal/success").into_response()),
                Err(e) => {
                    tracing::error!("session migration failed: {e}");
                    Ok(render(&PortalTemplate {
                        error: Some("portal.error.expired".to_string()),
                        csrf_token: state.portal_csrf_store.generate(&client.mac),
                    })?.into_response())
                }
            }
        }
        TokenLookup::Invalid => {
            Ok(render(&PortalTemplate {
                error: Some("portal.error.invalid".to_string()),
                csrf_token: state.portal_csrf_store.generate(&client.mac),
            })?.into_response())
        }
    }
}

/// GET /portal/success -- "you're connected" page
async fn success_page(
    State(state): State<Arc<AppState>>,
    client: ClientInfo,
) -> Result<impl IntoResponse, AppError> {
    let remaining_seconds = get_remaining_seconds(&state, &client.mac).await?;
    match remaining_seconds {
        Some(secs) => Ok(render(&SuccessTemplate { remaining_seconds: secs })?.into_response()),
        None => Ok(Redirect::to("/portal").into_response()),
    }
}

/// GET /portal/expired -- "session expired" page
async fn expired_page() -> Result<impl IntoResponse, AppError> {
    render(&ExpiredTemplate)
}

// -- Status API --

/// JSON response for `/portal/status`.
#[derive(Serialize)]
struct PortalStatusResponse {
    connected: bool,
    remaining_seconds: i64,
}

/// GET /portal/status -- current session status for this client (by MAC).
///
/// Returns JSON so the success page JS countdown can sync with the server,
/// and so CPD probes can check connectivity state.
async fn portal_status(
    State(state): State<Arc<AppState>>,
    client: ClientInfo,
) -> Result<impl IntoResponse, AppError> {
    let session = state.db.get_session_by_mac(&client.mac).await
        .map_err(AppError::Internal)?;

    let response = match session {
        Some(s) => {
            let remaining = s.remaining_seconds();
            PortalStatusResponse {
                connected: remaining > 0,
                remaining_seconds: remaining,
            }
        }
        None => PortalStatusResponse {
            connected: false,
            remaining_seconds: 0,
        },
    };

    Ok(Json(response))
}

/// GET /portal/privacy — privacy notice
async fn privacy_page() -> Result<impl IntoResponse, AppError> {
    render(&PrivacyTemplate)
}

/// GET /portal/plans — public page listing active plans and contact info
async fn plans_page(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let plans = state.db.list_active_plans().await
        .map_err(AppError::Internal)?;
    let portal = &state.config.portal;
    render(&PlansTemplate {
        plans,
        cafe_name: portal.cafe_name.clone(),
        contact_phone: portal.contact_phone.clone(),
        contact_name: portal.contact_name.clone(),
        contact_hours: portal.contact_hours.clone(),
    })
}

// -- Shared helpers --

/// Get remaining seconds for a session by MAC address.
/// Returns `None` if no active session exists.
async fn get_remaining_seconds(state: &Arc<AppState>, mac: &str) -> Result<Option<i64>, AppError> {
    let session = state.db.get_session_by_mac(mac).await
        .map_err(AppError::Internal)?;

    match session {
        Some(s) => {
            let secs = s.remaining_seconds();
            if secs > 0 {
                Ok(Some(secs))
            } else {
                Ok(None)
            }
        }
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use axum::http::StatusCode;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use crate::test_utils::{test_state, test_get, test_post_form};
    use crate::firewall::MockFirewall;

    /// Build the portal router for testing.
    fn portal_router(state: std::sync::Arc<crate::AppState>) -> axum::Router {
        super::routes().with_state(state)
    }

    #[tokio::test]
    async fn test_get_portal_returns_200() {
        let state = test_state().await;
        let app = portal_router(state);

        let response = app.oneshot(test_get("/portal")).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("DidiCafe"));
        assert!(html.contains("/portal/auth")); // form action
    }

    #[tokio::test]
    async fn test_portal_auth_invalid_format() {
        let state = test_state().await;
        // Seed CSRF store for the test client MAC
        state.portal_csrf_store.set("02:00:00:00:00:01", "test-csrf");
        let app = portal_router(state);

        let response = app
            .oneshot(test_post_form("/portal/auth", "token=BADTOKEN&csrf_token=test-csrf"))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK); // Returns form with error
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("portal.error.invalid"));
    }

    #[tokio::test]
    async fn test_portal_auth_valid_format_nonexistent_token() {
        let state = test_state().await;
        state.portal_csrf_store.set("02:00:00:00:00:01", "test-csrf");
        let app = portal_router(state);

        let response = app
            .oneshot(test_post_form("/portal/auth", "token=DIDI-ABCD-EF23&csrf_token=test-csrf"))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("portal.error.invalid"));
    }

    #[tokio::test]
    async fn test_full_portal_auth_flow() {
        let state = test_state().await;

        // Setup: create plan + token
        let plan_id = state.db.create_plan("1h WiFi", 60, 1000).await.unwrap();
        state.db.create_token("DIDI-ABCD-EF23", None, plan_id).await.unwrap();

        // Seed CSRF store with a known token
        state.portal_csrf_store.set("02:00:00:00:00:01", "my-csrf-token");

        // POST valid token with correct CSRF
        let app = portal_router(Arc::clone(&state));
        let response = app
            .oneshot(test_post_form("/portal/auth", "token=DIDI-ABCD-EF23&csrf_token=my-csrf-token"))
            .await
            .unwrap();

        // Should redirect to /portal/success
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            response.headers().get("location").unwrap().to_str().unwrap(),
            "/portal/success"
        );

        // Verify: token is now active
        let token = state.db.get_token_by_code("DIDI-ABCD-EF23").await.unwrap().unwrap();
        assert_eq!(token.status, crate::db::TokenStatus::Active);

        // Verify: session was created
        let sessions = state.db.get_active_sessions().await.unwrap();
        assert_eq!(sessions.len(), 1);

        // Verify: firewall was called
        let fw = state.firewall.as_ref();
        let mock = fw.as_any().downcast_ref::<MockFirewall>()
            .expect("firewall should be MockFirewall in tests");
        let calls = mock.calls();
        assert_eq!(calls.len(), 1);
    }

    #[tokio::test]
    async fn test_portal_auth_csrf_invalid() {
        let state = test_state().await;
        // Seed with a different token than what we'll submit
        state.portal_csrf_store.set("02:00:00:00:00:01", "correct-token");
        let app = portal_router(state);

        let response = app
            .oneshot(test_post_form("/portal/auth", "token=DIDI-ABCD-EF23&csrf_token=wrong-token"))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("portal.error.csrf"));
    }

    #[tokio::test]
    async fn test_portal_auth_csrf_missing() {
        let state = test_state().await;
        // Don't seed CSRF store at all
        let app = portal_router(state);

        let response = app
            .oneshot(test_post_form("/portal/auth", "token=DIDI-ABCD-EF23&csrf_token=whatever"))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("portal.error.csrf"));
    }

    #[tokio::test]
    async fn test_portal_status_no_session() {
        let state = test_state().await;
        let app = portal_router(state);

        let response = app.oneshot(test_get("/portal/status")).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["connected"], false);
        assert_eq!(json["remaining_seconds"], 0);
    }

    #[tokio::test]
    async fn test_portal_expired_returns_200() {
        let state = test_state().await;
        let app = portal_router(state);

        let response = app.oneshot(test_get("/portal/expired")).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_portal_success_no_session_redirects() {
        let state = test_state().await;
        let app = portal_router(state);

        let response = app.oneshot(test_get("/portal/success")).await.unwrap();

        // No session -> redirect to /portal
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            response.headers().get("location").unwrap().to_str().unwrap(),
            "/portal"
        );
    }

    #[tokio::test]
    async fn test_portal_auth_active_token_migrates_session() {
        let state = test_state().await;

        // Setup: create plan + token + first session (simulates initial connect)
        let plan_id = state.db.create_plan("1h WiFi", 60, 1000).await.unwrap();
        let token_id = state.db.create_token("DIDI-ABCD-EF23", None, plan_id).await.unwrap();

        // Simulate first session on old MAC
        crate::services::session::create_session(
            &state, token_id, 60, "aa:bb:cc:dd:ee:01", "10.10.0.5",
        ).await.unwrap();

        // Verify: 1 active session, token is active
        let sessions = state.db.get_active_sessions().await.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].mac_address, "aa:bb:cc:dd:ee:01");

        // Now client reconnects with new MAC (randomized), re-enters same token
        state.portal_csrf_store.set("02:00:00:00:00:01", "csrf-migrate");
        let app = portal_router(Arc::clone(&state));
        let response = app
            .oneshot(test_post_form("/portal/auth", "token=DIDI-ABCD-EF23&csrf_token=csrf-migrate"))
            .await
            .unwrap();

        // Should redirect to /portal/success (migration succeeded)
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            response.headers().get("location").unwrap().to_str().unwrap(),
            "/portal/success"
        );

        // Verify: old session disconnected, new session created on test MAC
        let sessions = state.db.get_active_sessions().await.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].mac_address, "02:00:00:00:00:01");

        // Verify: token is still active (not consumed twice)
        let token = state.db.get_token_by_code("DIDI-ABCD-EF23").await.unwrap().unwrap();
        assert_eq!(token.status, crate::db::TokenStatus::Active);

        // Verify: firewall was called (authorize old + deauthorize old + authorize new)
        let fw = state.firewall.as_ref();
        let mock = fw.as_any().downcast_ref::<MockFirewall>()
            .expect("firewall should be MockFirewall in tests");
        let calls = mock.calls();
        // 1: authorize_mac(old), 2: deauthorize_mac(old), 3: authorize_mac(new)
        assert_eq!(calls.len(), 3);
    }

    #[tokio::test]
    async fn test_portal_auth_expired_token_rejected() {
        let state = test_state().await;

        // Setup: create plan + token, then expire it
        let plan_id = state.db.create_plan("1h WiFi", 60, 1000).await.unwrap();
        let token_id = state.db.create_token("DIDI-EXPD-TK23", None, plan_id).await.unwrap();
        state.db.redeem_token(token_id, "2020-01-01 00:00:00").await.unwrap();
        state.db.expire_token(token_id).await.unwrap();

        state.portal_csrf_store.set("02:00:00:00:00:01", "csrf-expired");
        let app = portal_router(Arc::clone(&state));
        let response = app
            .oneshot(test_post_form("/portal/auth", "token=DIDI-EXPD-TK23&csrf_token=csrf-expired"))
            .await
            .unwrap();

        // Expired tokens should be rejected
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("portal.error.invalid"));
    }

    // -- Plans page tests --

    #[tokio::test]
    async fn test_plans_page_returns_200() {
        let state = test_state().await;
        let app = portal_router(state);

        let response = app.oneshot(test_get("/portal/plans")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("plans.title"));
    }

    #[tokio::test]
    async fn test_plans_page_shows_active_plans() {
        let state = test_state().await;

        // Create two active plans and one inactive
        state.db.create_plan("30min WiFi", 30, 500).await.unwrap();
        state.db.create_plan("1h WiFi", 60, 1000).await.unwrap();
        let id3 = state.db.create_plan("2h WiFi", 120, 2000).await.unwrap();
        state.db.update_plan(id3, "2h WiFi", 120, 2000, false).await.unwrap();

        let app = portal_router(Arc::clone(&state));
        let response = app.oneshot(test_get("/portal/plans")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let html = String::from_utf8_lossy(&body);
        // Active plans appear
        assert!(html.contains("30min WiFi"));
        assert!(html.contains("1h WiFi"));
        // Inactive plan does NOT appear
        assert!(!html.contains("2h WiFi"));
        // Price is formatted with Ariary suffix
        assert!(html.contains("Ar"));
    }

    #[tokio::test]
    async fn test_plans_page_empty() {
        let state = test_state().await;
        let app = portal_router(state);

        let response = app.oneshot(test_get("/portal/plans")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let html = String::from_utf8_lossy(&body);
        // Should show the "no plans" message
        assert!(html.contains("plans.no_plans"));
    }

    // -- fmt_ariary / format_ariary unit tests --

    #[test]
    fn test_format_ariary_basic() {
        // Small number — no grouping
        assert_eq!(super::format_ariary("500"), "500");

        // Thousands grouping with narrow no-break space
        assert_eq!(super::format_ariary("1000"), "1\u{202f}000");
        assert_eq!(super::format_ariary("10000"), "10\u{202f}000");
        assert_eq!(super::format_ariary("100000"), "100\u{202f}000");
        assert_eq!(super::format_ariary("1000000"), "1\u{202f}000\u{202f}000");

        // Zero
        assert_eq!(super::format_ariary("0"), "0");

        // Negative number
        assert_eq!(super::format_ariary("-5000"), "-5\u{202f}000");

        // Non-numeric string passed through as-is
        assert_eq!(super::format_ariary("hello"), "hello");

        // Empty string
        assert_eq!(super::format_ariary(""), "");

        // Single digit
        assert_eq!(super::format_ariary("7"), "7");

        // Exact boundary (3 digits — no separator needed)
        assert_eq!(super::format_ariary("999"), "999");
    }
}
