use std::net::SocketAddr;
use std::sync::Arc;
use axum::{
    Form,
    Router,
    extract::{ConnectInfo, State},
    http::{StatusCode, header::SET_COOKIE},
    response::{AppendHeaders, IntoResponse, Redirect},
    routing::{get, post},
};
use askama::Template;
use serde::Deserialize;
use subtle::ConstantTimeEq;

use crate::AppState;
use crate::services::rate_limit::RateLimitResult;
use super::error::{AppError, render};
use super::extractors::{AdminSession, CsrfToken, extract_cookie, ADMIN_COOKIE_NAME};

/// Maximum length for login form fields to prevent Argon2 DoS.
/// OWASP recommends limiting password length to prevent hash-flooding.
const MAX_USERNAME_LEN: usize = 256;
const MAX_PASSWORD_LEN: usize = 1024;

/// Verify a password against an Argon2id PHC hash string.
///
/// Uses Argon2id per OWASP recommendations:
/// https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html#argon2id
///
/// Runs on a blocking thread via `spawn_blocking` because Argon2id is
/// deliberately CPU/memory-intensive (19 MiB, 2 iterations). Running it
/// on a tokio worker thread would stall all other requests on that thread.
async fn verify_password(password: &str, hash: &str) -> bool {
    let password = password.to_owned();
    let hash = hash.to_owned();

    tokio::task::spawn_blocking(move || {
        use argon2::Argon2;
        use argon2::password_hash::{PasswordHash, PasswordVerifier};

        let Ok(parsed) = PasswordHash::new(&hash) else {
            return false;
        };
        Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok()
    })
    .await
    .unwrap_or_else(|e| {
        tracing::error!("password verification task failed: {e}");
        false
    })
}

/// Build a Set-Cookie header value for the admin session.
///
/// Flags per OWASP Session Management Cheat Sheet:
/// - `HttpOnly`: prevents JavaScript access (XSS mitigation)
/// - `SameSite=Strict`: prevents CSRF via cross-site requests
/// - `Path=/`: required for both /admin and /api paths
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
    csrf_token: String,
}

#[derive(Template)]
#[template(path = "admin/manage.html")]
struct ManageTemplate {
    tokens: Vec<crate::db::Token>,
    plans: Vec<crate::db::Plan>,
    sessions: Vec<crate::db::Session>,
    message: Option<String>,
    csrf_token: String,
}

#[derive(Template)]
#[template(path = "admin/audit.html")]
struct AuditTemplate {
    entries: Vec<crate::db::AuditLogEntry>,
}

// -- Forms --

#[derive(Deserialize)]
pub struct LoginForm {
    username: String,
    password: String,
}

#[derive(Deserialize)]
pub struct ManageForm {
    plan_id: Option<i64>,
    count: Option<usize>,
    name: Option<String>,
    duration: Option<i64>,
    price: Option<i64>,
    plan_name: Option<String>,
    csrf_token: String,
}

// -- Routes --

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/admin", get(dashboard))
        .route("/admin/", get(redirect_to_login))
        .route("/admin/login", get(login_page).post(login_submit))
        .route("/admin/logout", post(logout))
        .route("/admin/manage", get(manage_page).post(manage_submit))
        .route("/admin/audit", get(audit_page))
}

/// GET /admin — redirect to login if not authenticated
async fn redirect_to_login(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> axum::response::Response {
    if let Some(session_id) = extract_cookie(&headers, ADMIN_COOKIE_NAME)
        && state.admin_sessions.validate(&session_id)
    {
        return Redirect::to("/admin").into_response();
    }
    Redirect::to("/admin/login").into_response()
}

/// GET /admin/login — login page (no auth required)
async fn login_page() -> Result<impl IntoResponse, AppError> {
    render(&LoginTemplate { error: None })
}

/// POST /admin/login — validate credentials, create session, set cookie
async fn login_submit(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: axum::http::HeaderMap,
    Form(form): Form<LoginForm>,
) -> Result<impl IntoResponse, AppError> {
    // Rate limit admin login attempts (OWASP: all auth endpoints)
    match state.admin_rate_limiter.check_and_record(addr.ip()) {
        RateLimitResult::Banned { retry_after_seconds } => {
            let body = render(&LoginTemplate {
                error: Some("Too many attempts. Please wait and try again.".to_string()),
            })?;
            return Ok((
                StatusCode::TOO_MANY_REQUESTS,
                AppendHeaders([
                    ("Retry-After", retry_after_seconds.to_string()),
                ]),
                body,
            ).into_response());
        }
        RateLimitResult::Throttled => {
            let body = render(&LoginTemplate {
                error: Some("Too many attempts. Please wait and try again.".to_string()),
            })?;
            return Ok((StatusCode::TOO_MANY_REQUESTS, body).into_response());
        }
        RateLimitResult::Allowed => {}
    }

    // Input length limits: prevent Argon2 DoS with oversized passwords
    if form.username.len() > MAX_USERNAME_LEN || form.password.len() > MAX_PASSWORD_LEN {
        let body = render(&LoginTemplate {
            error: Some("Invalid credentials.".to_string()),
        })?;
        return Ok(body.into_response());
    }

    // Constant-time username comparison (prevents timing oracle)
    let username_ok = form.username.as_bytes()
        .ct_eq(state.config.admin.username.as_bytes())
        .into();
    let password_ok = verify_password(&form.password, &state.config.admin.password_hash).await;

    if username_ok && password_ok {
        state.db.audit_log(&form.username, "login", None, None, None).await.ok();

        // Session fixation fix: invalidate any existing session from this cookie
        if let Some(old_id) = extract_cookie(&headers, ADMIN_COOKIE_NAME) {
            state.admin_sessions.remove(&old_id);
        }

        let session_id = state.admin_sessions.create();
        let cookie = session_cookie(&session_id);
        Ok((
            AppendHeaders([(SET_COOKIE, cookie)]),
            Redirect::to("/admin"),
        ).into_response())
    } else {
        // Log failed login attempt (OWASP audit trail) — truncate username to prevent log flooding
        let logged_user = &form.username[..form.username.len().min(MAX_USERNAME_LEN)];
        state.db.audit_log(logged_user, "login_failed", None, None, None).await.ok();
        let body = render(&LoginTemplate {
            error: Some("Invalid credentials.".to_string()),
        })?;
        Ok(body.into_response())
    }
}

/// POST /admin/logout — invalidate session, clear cookie, redirect to login
async fn logout(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    // Extract session ID from cookie and invalidate server-side
    if let Some(session_id) =
        extract_cookie(&headers, ADMIN_COOKIE_NAME)
    {
        state.admin_sessions.remove(&session_id);
    }

    Ok((
        AppendHeaders([(SET_COOKIE, clear_session_cookie())]),
        Redirect::to("/admin/login"),
    ))
}

/// GET /admin — dashboard (requires auth)
async fn dashboard(
    State(state): State<Arc<AppState>>,
    _admin: AdminSession,
    csrf: CsrfToken,
) -> Result<impl IntoResponse, AppError> {
    let stats = state.db.get_daily_stats().await
        .map_err(AppError::Internal)?;
    let active_sessions = state.db.get_active_sessions().await
        .map_err(AppError::Internal)?;

    render(&DashboardTemplate {
        stats,
        active_sessions,
        csrf_token: csrf.0,
    })
}

/// GET /admin/manage — consolidated management page (requires auth)
async fn manage_page(
    State(state): State<Arc<AppState>>,
    _admin: AdminSession,
    csrf: CsrfToken,
) -> Result<impl IntoResponse, AppError> {
    let (tokens, plans, sessions) = load_manage_data(&state).await?;
    render(&ManageTemplate {
        tokens,
        plans,
        sessions,
        message: None,
        csrf_token: csrf.0,
    })
}

/// POST /admin/manage — handle token generation or plan creation (requires auth)
async fn manage_submit(
    State(state): State<Arc<AppState>>,
    _admin: AdminSession,
    csrf: CsrfToken,
    Form(form): Form<ManageForm>,
) -> Result<impl IntoResponse, AppError> {
    // CSRF validation (constant-time comparison)
    let csrf_ok: bool = form.csrf_token.as_bytes()
        .ct_eq(csrf.0.as_bytes())
        .into();
    if !csrf_ok {
        return Err(AppError::BadRequest("Invalid CSRF token".to_string()));
    }

    let message = if let Some(plan_name) = &form.plan_name {
        if !plan_name.is_empty() {
            let duration = form.duration
                .ok_or_else(|| AppError::BadRequest("duration is required".to_string()))?;
            let price = form.price
                .ok_or_else(|| AppError::BadRequest("price is required".to_string()))?;
            validate_plan_input(plan_name, duration, price)?;
            state.db.create_plan(plan_name, duration, price).await
                .map_err(AppError::Internal)?;
            state.db.audit_log(&state.config.admin.username, "create_plan", Some("plan"), None, Some(plan_name)).await.ok();
            Some(format!("Plan '{}' created.", plan_name))
        } else {
            None
        }
    } else if let Some(plan_id) = form.plan_id {
        let count = form.count.unwrap_or(0);
        if count > 0 {
            let token_name = form.name.as_deref().unwrap_or_default();
            if token_name.len() > 200 {
                return Err(AppError::BadRequest("name must be 200 characters or less".to_string()));
            }
            let codes = crate::services::token::generate_tokens(
                &state.db,
                &state.config.token,
                plan_id,
                count,
                Some(token_name),
            ).await.map_err(AppError::Internal)?;
            state.db.audit_log(&state.config.admin.username, "generate_tokens", Some("token"), Some(plan_id), Some(&format!("{} tokens for plan {}", codes.len(), plan_id))).await.ok();
            Some(format!("{} token(s) generated", codes.len()))
        } else {
            None
        }
    } else {
        None
    };

    let (tokens, plans, sessions) = load_manage_data(&state).await?;
    render(&ManageTemplate {
        tokens,
        plans,
        sessions,
        message,
        csrf_token: csrf.0,
    })
}

// -- Shared helpers --

/// Validate plan input fields. Returns `Err(BadRequest)` on invalid data.
pub fn validate_plan_input(name: &str, duration: i64, price: i64) -> Result<(), AppError> {
    if name.is_empty() {
        return Err(AppError::BadRequest("name must not be empty".to_string()));
    }
    if name.len() > 100 {
        return Err(AppError::BadRequest("name must be 100 characters or less".to_string()));
    }
    if duration <= 0 {
        return Err(AppError::BadRequest("duration must be > 0".to_string()));
    }
    if duration > 1440 {
        return Err(AppError::BadRequest("duration must be <= 1440 minutes (24h)".to_string()));
    }
    if price < 0 {
        return Err(AppError::BadRequest("price must be >= 0".to_string()));
    }
    Ok(())
}

/// Load all manage page data (tokens, plans, sessions) in one place.
async fn load_manage_data(
    state: &Arc<AppState>,
) -> Result<(Vec<crate::db::Token>, Vec<crate::db::Plan>, Vec<crate::db::Session>), AppError> {
    let tokens = state.db.list_tokens(None).await.map_err(AppError::Internal)?;
    let plans = state.db.list_plans().await.map_err(AppError::Internal)?;
    let sessions = state.db.get_active_sessions().await.map_err(AppError::Internal)?;
    Ok((tokens, plans, sessions))
}

/// GET /admin/audit — audit log viewer (requires auth)
async fn audit_page(
    State(state): State<Arc<AppState>>,
    _admin: AdminSession,
) -> Result<impl IntoResponse, AppError> {
    let entries = state.db.get_audit_log(200).await
        .map_err(AppError::Internal)?;
    render(&AuditTemplate { entries })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_argon2id_hash_and_verify() {
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
        assert!(verify_password("changeme", &hash).await);
        assert!(!verify_password("wrongpassword", &hash).await);
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
