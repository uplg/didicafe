use std::sync::Arc;
use axum::{
    Form,
    Router,
    extract::State,
    http::header::SET_COOKIE,
    response::{AppendHeaders, IntoResponse, Redirect},
    routing::{get, post},
};
use askama::Template;
use serde::Deserialize;

use crate::AppState;
use super::error::{AppError, render};
use super::extractors::{AdminSession, ADMIN_COOKIE_NAME};

/// Verify a password against an Argon2id PHC hash string.
///
/// Uses Argon2id per OWASP recommendations:
/// https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html#argon2id
fn verify_password(password: &str, hash: &str) -> bool {
    use argon2::Argon2;
    use argon2::password_hash::{PasswordHash, PasswordVerifier};

    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok()
}

/// Build a Set-Cookie header value for the admin session.
///
/// Flags per OWASP Session Management Cheat Sheet:
/// - `HttpOnly`: prevents JavaScript access (XSS mitigation)
/// - `SameSite=Strict`: prevents CSRF via cross-site requests
/// - `Path=/admin` + `/api`: scoped to admin/API paths only
/// - `Secure`: omitted — the captive portal runs over HTTP on a local network
fn session_cookie(session_id: &str) -> String {
    format!(
        "{ADMIN_COOKIE_NAME}={session_id}; HttpOnly; SameSite=Strict; Path=/"
    )
}

/// Build a Set-Cookie header that clears the admin session cookie.
fn clear_session_cookie() -> String {
    format!(
        "{ADMIN_COOKIE_NAME}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0"
    )
}

// -- Templates --

#[derive(Template)]
#[template(path = "admin/login.html")]
struct LoginTemplate {
    error: Option<String>,
}

#[derive(Template)]
#[template(path = "admin/dashboard.html")]
struct DashboardTemplate {
    stats: crate::db::DailyStats,
    active_sessions: Vec<crate::db::Session>,
}

#[derive(Template)]
#[template(path = "admin/tokens.html")]
struct TokensTemplate {
    tokens: Vec<crate::db::Token>,
    plans: Vec<crate::db::Plan>,
    generated: Vec<String>,
}

#[derive(Template)]
#[template(path = "admin/plans.html")]
struct PlansTemplate {
    plans: Vec<crate::db::Plan>,
    message: Option<String>,
}

#[derive(Template)]
#[template(path = "admin/sessions.html")]
struct SessionsTemplate {
    sessions: Vec<crate::db::Session>,
}

// -- Forms --

#[derive(Deserialize)]
pub struct LoginForm {
    username: String,
    password: String,
}

#[derive(Deserialize)]
pub struct GenerateTokensForm {
    plan_id: i64,
    count: usize,
}

#[derive(Deserialize)]
pub struct CreatePlanForm {
    name: String,
    duration_minutes: i64,
    price_ariary: i64,
}

// -- Routes --

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/admin", get(dashboard))
        .route("/admin/login", get(login_page).post(login_submit))
        .route("/admin/logout", post(logout))
        .route("/admin/tokens", get(tokens_page).post(generate_tokens))
        .route("/admin/plans", get(plans_page).post(create_plan))
        .route("/admin/sessions", get(sessions_page))
}

/// GET /admin/login — login page (no auth required)
async fn login_page() -> Result<impl IntoResponse, AppError> {
    render(&LoginTemplate { error: None })
}

/// POST /admin/login — validate credentials, create session, set cookie
async fn login_submit(
    State(state): State<Arc<AppState>>,
    Form(form): Form<LoginForm>,
) -> Result<impl IntoResponse, AppError> {
    if form.username == state.config.admin.username
        && verify_password(&form.password, &state.config.admin.password_hash)
    {
        let session_id = state.admin_sessions.create();
        let cookie = session_cookie(&session_id);
        Ok((
            AppendHeaders([(SET_COOKIE, cookie)]),
            Redirect::to("/admin"),
        ).into_response())
    } else {
        Ok(render(&LoginTemplate {
            error: Some("Invalid credentials.".to_string()),
        })?.into_response())
    }
}

/// POST /admin/logout — invalidate session, clear cookie, redirect to login
async fn logout(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    // Extract session ID from cookie and invalidate server-side
    if let Some(session_id) =
        super::extractors::extract_cookie(&headers, ADMIN_COOKIE_NAME)
    {
        state.admin_sessions.remove(&session_id);
    }

    (
        AppendHeaders([(SET_COOKIE, clear_session_cookie())]),
        Redirect::to("/admin/login"),
    )
}

/// GET /admin — dashboard (requires auth)
async fn dashboard(
    State(state): State<Arc<AppState>>,
    _admin: AdminSession,
) -> Result<impl IntoResponse, AppError> {
    let stats = state.db.get_daily_stats().await
        .map_err(AppError::Internal)?;
    let active_sessions = state.db.get_active_sessions().await
        .map_err(AppError::Internal)?;

    render(&DashboardTemplate {
        stats,
        active_sessions,
    })
}

/// GET /admin/tokens — token management (requires auth)
async fn tokens_page(
    State(state): State<Arc<AppState>>,
    _admin: AdminSession,
) -> Result<impl IntoResponse, AppError> {
    let tokens = state.db.list_tokens(None).await
        .map_err(AppError::Internal)?;
    let plans = state.db.list_plans().await
        .map_err(AppError::Internal)?;

    render(&TokensTemplate {
        tokens,
        plans,
        generated: vec![],
    })
}

/// POST /admin/tokens — generate new tokens (requires auth)
async fn generate_tokens(
    State(state): State<Arc<AppState>>,
    _admin: AdminSession,
    Form(form): Form<GenerateTokensForm>,
) -> Result<impl IntoResponse, AppError> {
    let generated = crate::services::token::generate_tokens(
        &state.db,
        &state.config.token,
        form.plan_id,
        form.count,
    ).await.map_err(AppError::Internal)?;

    let tokens = state.db.list_tokens(None).await
        .map_err(AppError::Internal)?;
    let plans = state.db.list_plans().await
        .map_err(AppError::Internal)?;

    render(&TokensTemplate {
        tokens,
        plans,
        generated,
    })
}

/// GET /admin/plans — plan management (requires auth)
async fn plans_page(
    State(state): State<Arc<AppState>>,
    _admin: AdminSession,
) -> Result<impl IntoResponse, AppError> {
    let plans = state.db.list_plans().await
        .map_err(AppError::Internal)?;
    render(&PlansTemplate { plans, message: None })
}

/// POST /admin/plans — create a new plan (requires auth)
async fn create_plan(
    State(state): State<Arc<AppState>>,
    _admin: AdminSession,
    Form(form): Form<CreatePlanForm>,
) -> Result<impl IntoResponse, AppError> {
    if form.name.is_empty() {
        return Err(AppError::BadRequest("name must not be empty".to_string()));
    }
    if form.duration_minutes <= 0 {
        return Err(AppError::BadRequest("duration_minutes must be > 0".to_string()));
    }

    state.db.create_plan(&form.name, form.duration_minutes, form.price_ariary).await
        .map_err(AppError::Internal)?;

    let plans = state.db.list_plans().await
        .map_err(AppError::Internal)?;
    render(&PlansTemplate {
        plans,
        message: Some(format!("Plan '{}' created.", form.name)),
    })
}

/// GET /admin/sessions — session management (requires auth)
async fn sessions_page(
    State(state): State<Arc<AppState>>,
    _admin: AdminSession,
) -> Result<impl IntoResponse, AppError> {
    let sessions = state.db.get_active_sessions().await
        .map_err(AppError::Internal)?;
    render(&SessionsTemplate { sessions })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_argon2id_hash_and_verify() {
        use argon2::{Argon2, Params};
        use argon2::password_hash::{PasswordHasher, SaltString, rand_core::OsRng};

        // OWASP minimum: m=19456 (19 MiB), t=2, p=1
        let params = Params::new(19456, 2, 1, None).unwrap();
        let argon2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);

        let salt = SaltString::generate(&mut OsRng);
        let hash = argon2.hash_password(b"changeme", &salt).unwrap().to_string();

        // Print for config file generation
        println!("Argon2id hash for 'changeme': {hash}");

        assert!(hash.starts_with("$argon2id$"));
        assert!(verify_password("changeme", &hash));
        assert!(!verify_password("wrongpassword", &hash));
    }

    #[test]
    fn test_session_cookie_format() {
        let cookie = session_cookie("test-session-id");
        assert!(cookie.contains("didicafe_admin=test-session-id"));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Strict"));
    }

    #[test]
    fn test_clear_cookie_format() {
        let cookie = clear_session_cookie();
        assert!(cookie.contains("didicafe_admin="));
        assert!(cookie.contains("Max-Age=0"));
        assert!(cookie.contains("HttpOnly"));
    }
}
