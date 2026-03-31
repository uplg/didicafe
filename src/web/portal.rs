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
use crate::services;
use crate::services::rate_limit::RateLimitResult;
use super::error::{AppError, render};
use super::extractors::ClientInfo;

// -- Templates --

#[derive(Template)]
#[template(path = "portal.html")]
struct PortalTemplate {
    error: Option<String>,
}

#[derive(Template)]
#[template(path = "success.html")]
struct SuccessTemplate {
    remaining_minutes: i64,
}

#[derive(Template)]
#[template(path = "expired.html")]
struct ExpiredTemplate;

#[derive(Template)]
#[template(path = "privacy.html")]
struct PrivacyTemplate;

// -- Form --

#[derive(Deserialize)]
pub struct AuthForm {
    token: String,
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
}

/// GET /health -- health check for process supervision
async fn health_check(State(state): State<Arc<AppState>>) -> axum::http::StatusCode {
    match state.db.get_daily_stats().await {
        Ok(_) => axum::http::StatusCode::OK,
        Err(_) => axum::http::StatusCode::SERVICE_UNAVAILABLE,
    }
}

/// GET /portal -- splash page with token input
async fn portal_page() -> Result<impl IntoResponse, AppError> {
    render(&PortalTemplate { error: None })
}

/// POST /portal/auth -- validate token, create session
async fn portal_auth(
    State(state): State<Arc<AppState>>,
    client: ClientInfo,
    Form(form): Form<AuthForm>,
) -> Result<impl IntoResponse, AppError> {
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
    if let Err(msg) = services::token::validate_token_format(&state.config.token, &code) {
        return Ok(render(&PortalTemplate {
            error: Some(msg),
        })?.into_response());
    }

    // Validate token against DB
    let token = services::token::validate_token(&state.db, &code).await
        .map_err(AppError::Internal)?;

    let Some(token) = token else {
        return Ok(render(&PortalTemplate {
            error: Some("Invalid or already used token.".to_string()),
        })?.into_response());
    };

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
                error: Some("Internal error. Please try again.".to_string()),
            })?.into_response())
        }
    }
}

/// GET /portal/success -- "you're connected" page
async fn success_page(
    State(state): State<Arc<AppState>>,
    client: ClientInfo,
) -> Result<impl IntoResponse, AppError> {
    let session = state.db.get_session_by_mac(&client.mac).await
        .map_err(AppError::Internal)?;

    let remaining_minutes = match session {
        Some(s) => {
            let secs = s.remaining_seconds();
            // Round up so the user never sees "0 minutes" when there's still time
            (secs + 59) / 60
        }
        None => {
            // No active session — redirect to portal
            return Ok(Redirect::to("/portal").into_response());
        }
    };

    Ok(render(&SuccessTemplate { remaining_minutes })?.into_response())
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
        let app = portal_router(state);

        let response = app
            .oneshot(test_post_form("/portal/auth", "token=BADTOKEN"))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK); // Returns form with error
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("Invalid token format"));
    }

    #[tokio::test]
    async fn test_portal_auth_valid_format_nonexistent_token() {
        let state = test_state().await;
        let app = portal_router(state);

        let response = app
            .oneshot(test_post_form("/portal/auth", "token=DIDI-ABCD-EF23"))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("Invalid or already used token"));
    }

    #[tokio::test]
    async fn test_full_portal_auth_flow() {
        let state = test_state().await;

        // Setup: create plan + token
        let plan_id = state.db.create_plan("1h WiFi", 60, 1000).await.unwrap();
        state.db.create_token("DIDI-ABCD-EF23", None, plan_id).await.unwrap();

        // POST valid token
        let app = portal_router(Arc::clone(&state));
        let response = app
            .oneshot(test_post_form("/portal/auth", "token=DIDI-ABCD-EF23"))
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
        assert_eq!(token.status, "active");

        // Verify: session was created with the mock MAC (unknown = "unknown")
        let sessions = state.db.get_active_sessions().await.unwrap();
        assert_eq!(sessions.len(), 1);

        // Verify: firewall was called
        let fw = state.firewall.as_ref();
        // Downcast to MockFirewall to check calls
        let mock = unsafe { &*(fw as *const dyn crate::firewall::Firewall as *const MockFirewall) };
        let calls = mock.calls();
        assert_eq!(calls.len(), 1);
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
}
