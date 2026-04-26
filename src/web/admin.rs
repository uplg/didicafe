use std::net::SocketAddr;
use std::sync::Arc;
use axum::{
    Form,
    Router,
    extract::{ConnectInfo, Query, State},
    http::{StatusCode, header::SET_COOKIE},
    response::{AppendHeaders, IntoResponse, Redirect},
    routing::{get, post},
};
use askama::Template;
use serde::Deserialize;
use subtle::ConstantTimeEq;

use crate::AppState;
use crate::config::PortalConfig;
use crate::services::rate_limit::RateLimitResult;
use super::error::{AppError, render};
use super::extractors::{AdminSession, CsrfToken, extract_cookie, ADMIN_COOKIE_NAME};
use super::I18nMessage;

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
/// - `Secure`: set when TLS is enabled (admin served over HTTPS)
fn session_cookie(session_id: &str, tls_enabled: bool) -> String {
    let secure = if tls_enabled { "; Secure" } else { "" };
    format!(
        "{ADMIN_COOKIE_NAME}={session_id}; HttpOnly; SameSite=Strict; Path=/{secure}"
    )
}

/// Build a Set-Cookie header that clears the admin session cookie.
fn clear_session_cookie(tls_enabled: bool) -> String {
    let secure = if tls_enabled { "; Secure" } else { "" };
    format!(
        "{ADMIN_COOKIE_NAME}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0{secure}"
    )
}

// -- Templates --

#[derive(Template)]
#[template(path = "admin/login.html")]
struct LoginTemplate {
    error: Option<String>,
    csrf_token: String,
    cafe_name: String,
    theme_css: String,
}

#[derive(Template)]
#[template(path = "admin/dashboard.html")]
struct DashboardTemplate {
    stats: crate::db::DailyStats,
    active_sessions: Vec<crate::db::Session>,
    csrf_token: String,
    cafe_name: String,
    current_page: String,
    theme_css: String,
}

#[derive(Template)]
#[template(path = "admin/manage.html")]
struct ManageTemplate {
    token_page: crate::db::TokenPage,
    plans: Vec<crate::db::Plan>,
    sessions: Vec<crate::db::Session>,
    message: Option<I18nMessage>,
    csrf_token: String,
    cafe_name: String,
    current_page: String,
    theme_css: String,
}

#[derive(Template)]
#[template(path = "admin/audit.html")]
struct AuditTemplate {
    audit_page: crate::db::AuditPage,
    cafe_name: String,
    current_page: String,
    theme_css: String,
}

#[derive(Template)]
#[template(path = "admin/settings.html")]
struct SettingsTemplate {
    current: PortalConfig,
    message: Option<String>,
    csrf_token: String,
    cafe_name: String,
    current_page: String,
    theme_css: String,
}

// -- Forms --

/// Tokens per page in the manage view.
const TOKENS_PER_PAGE: i64 = 50;

/// Audit log entries per page.
const AUDIT_PER_PAGE: i64 = 50;

#[derive(Deserialize)]
pub struct ManageQuery {
    /// Token status filter: "all", "current" (default), "unused", "active", "expired", "revoked"
    token_status: Option<String>,
    /// 1-indexed page number (default: 1)
    token_page: Option<i64>,
}

#[derive(Deserialize)]
pub struct AuditQuery {
    /// 1-indexed page number (default: 1)
    page: Option<i64>,
}

#[derive(Deserialize)]
pub struct LoginForm {
    username: String,
    password: String,
    csrf_token: String,
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

#[derive(Deserialize)]
pub struct SettingsForm {
    cafe_name: String,
    welcome_message: String,
    theme_color: String,
    contact_name: String,
    contact_phone: String,
    contact_hours: String,
    csrf_token: String,
}

// -- Routes --

/// Helper: get common admin template fields from state.
/// Reads settings from DB, falling back to TOML config values.
struct AdminCtx {
    cafe_name: String,
    theme_css: String,
}

async fn admin_ctx(state: &Arc<AppState>) -> AdminCtx {
    let portal = portal_config_from_db(state).await;
    AdminCtx {
        cafe_name: portal.cafe_name.clone(),
        theme_css: portal.generate_theme_css(),
    }
}

/// Build a `PortalConfig` by overlaying DB settings on top of TOML defaults.
/// Each setting key maps 1:1 to a `PortalConfig` field.
///
/// Used by both admin and portal handlers to get the current effective config.
pub(super) async fn portal_config_from_db(state: &Arc<AppState>) -> PortalConfig {
    let base = &state.config.portal;
    let settings = state.db.get_all_settings().await.unwrap_or_default();

    let mut cfg = base.clone();
    for (key, value) in &settings {
        match key.as_str() {
            "cafe_name" if !value.is_empty() => cfg.cafe_name = value.clone(),
            "welcome_message" => cfg.welcome_message = value.clone(),
            "theme_color" if crate::config::is_valid_hex_color(value) => {
                cfg.theme_color = value.clone();
            }
            "contact_name" => cfg.contact_name = value.clone(),
            "contact_phone" => cfg.contact_phone = value.clone(),
            "contact_hours" => cfg.contact_hours = value.clone(),
            _ => {}
        }
    }
    cfg
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/admin", get(dashboard))
        .route("/admin/", get(redirect_to_login))
        .route("/admin/login", get(login_page).post(login_submit))
        .route("/admin/logout", post(logout))
        .route("/admin/manage", get(manage_page).post(manage_submit))
        .route("/admin/audit", get(audit_page))
        .route("/admin/settings", get(settings_page).post(settings_submit))
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
///
/// CSRF protection: Synchronizer Token Pattern (server-side).
/// Token stored in `login_csrf_store` keyed by client IP, embedded in form.
async fn login_page(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Result<impl IntoResponse, AppError> {
    let ip_key = addr.ip().to_string();
    // get_or_generate (not generate) so back-to-back GETs from browsers
    // (background polls, prefetch, retry) don't invalidate the token rendered
    // on the first response.
    let csrf_token = state.login_csrf_store.get_or_generate(&ip_key);
    let ctx = admin_ctx(&state).await;
    render(&LoginTemplate { error: None, csrf_token, cafe_name: ctx.cafe_name, theme_css: ctx.theme_css })
}

/// POST /admin/login — validate credentials, create session, set cookie
///
/// CSRF protection: Synchronizer Token Pattern. The form-submitted `csrf_token`
/// is validated against the server-side token stored for this client IP.
async fn login_submit(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: axum::http::HeaderMap,
    Form(form): Form<LoginForm>,
) -> Result<impl IntoResponse, AppError> {
    let ip_key = addr.ip().to_string();
    let ctx = admin_ctx(&state).await;

    // Helper closure: build login error template
    let login_err = |error: &str, csrf: String| -> LoginTemplate {
        LoginTemplate {
            error: Some(error.to_string()),
            csrf_token: csrf,
            cafe_name: ctx.cafe_name.clone(),
            theme_css: ctx.theme_css.clone(),
        }
    };

    // CSRF validation: Synchronizer Token Pattern (server-side, one-time use)
    if !state.login_csrf_store.validate(&ip_key, &form.csrf_token) {
        let csrf_token = state.login_csrf_store.generate(&ip_key);
        let body = render(&login_err("admin.login.error.csrf", csrf_token))?;
        return Ok(body.into_response());
    }

    // Rate limit admin login attempts (OWASP: all auth endpoints)
    match state.admin_rate_limiter.check_and_record(addr.ip()) {
        RateLimitResult::Banned { retry_after_seconds } => {
            let csrf_token = state.login_csrf_store.generate(&ip_key);
            let body = render(&login_err("admin.login.error.rate_limit", csrf_token))?;
            return Ok((
                StatusCode::TOO_MANY_REQUESTS,
                AppendHeaders([
                    ("Retry-After", retry_after_seconds.to_string()),
                ]),
                body,
            ).into_response());
        }
        RateLimitResult::Throttled => {
            let csrf_token = state.login_csrf_store.generate(&ip_key);
            let body = render(&login_err("admin.login.error.rate_limit", csrf_token))?;
            return Ok((StatusCode::TOO_MANY_REQUESTS, body).into_response());
        }
        RateLimitResult::Allowed => {}
    }

    // Input length limits: prevent Argon2 DoS with oversized passwords
    if form.username.len() > MAX_USERNAME_LEN || form.password.len() > MAX_PASSWORD_LEN {
        let csrf_token = state.login_csrf_store.generate(&ip_key);
        let body = render(&login_err("admin.login.error", csrf_token))?;
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
        let cookie = session_cookie(&session_id, state.config.tls.enabled);
        Ok((
            AppendHeaders([(SET_COOKIE, cookie)]),
            Redirect::to("/admin"),
        ).into_response())
    } else {
        // Log failed login attempt (OWASP audit trail) — truncate username to prevent log flooding
        let logged_user = &form.username[..form.username.len().min(MAX_USERNAME_LEN)];
        state.db.audit_log(logged_user, "login_failed", None, None, None).await.ok();
        let csrf_token = state.login_csrf_store.generate(&ip_key);
        let body = render(&login_err("admin.login.error", csrf_token))?;
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
        AppendHeaders([(SET_COOKIE, clear_session_cookie(state.config.tls.enabled))]),
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
    let ctx = admin_ctx(&state).await;

    render(&DashboardTemplate {
        stats,
        active_sessions,
        csrf_token: csrf.0,
        cafe_name: ctx.cafe_name,
        current_page: "dashboard".to_string(),
        theme_css: ctx.theme_css,
    })
}

/// GET /admin/manage — consolidated management page (requires auth)
async fn manage_page(
    State(state): State<Arc<AppState>>,
    _admin: AdminSession,
    csrf: CsrfToken,
    Query(query): Query<ManageQuery>,
) -> Result<impl IntoResponse, AppError> {
    let (status_filter, page) = parse_token_query(&query);
    let (token_page, plans, sessions) = load_manage_data(&state, &status_filter, page).await?;
    let ctx = admin_ctx(&state).await;
    render(&ManageTemplate {
        token_page,
        plans,
        sessions,
        message: None,
        csrf_token: csrf.0,
        cafe_name: ctx.cafe_name,
        current_page: "manage".to_string(),
        theme_css: ctx.theme_css,
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
        return Err(AppError::BadRequest("admin.error.csrf".to_string()));
    }

    let message = if let Some(plan_name) = &form.plan_name {
        if !plan_name.is_empty() {
            let duration = form.duration
                .ok_or_else(|| AppError::BadRequest("admin.error.duration_required".to_string()))?;
            let price = form.price
                .ok_or_else(|| AppError::BadRequest("admin.error.price_required".to_string()))?;
            validate_plan_input(plan_name, duration, price)?;
            state.db.create_plan(plan_name, duration, price).await
                .map_err(AppError::Internal)?;
            state.db.audit_log(&state.config.admin.username, "create_plan", Some("plan"), None, Some(plan_name)).await.ok();
            Some(I18nMessage::with_args("admin.manage.plan_created", [
                ("name", plan_name.clone()),
            ]))
        } else {
            None
        }
    } else if let Some(plan_id) = form.plan_id {
        let count = form.count.unwrap_or(0);
        if count > 0 {
            let token_name = form.name.as_deref().unwrap_or_default();
            if token_name.is_empty() {
                return Err(AppError::BadRequest("admin.error.name_empty".to_string()));
            }
            if token_name.len() > 200 {
                return Err(AppError::BadRequest("admin.error.name_too_long_200".to_string()));
            }
            if count > 100 {
                return Err(AppError::BadRequest("admin.error.count_max".to_string()));
            }
            // Verify plan exists and is active (consistent with API validation)
            let plan = state.db.get_plan(plan_id).await
                .map_err(AppError::Internal)?
                .ok_or_else(|| AppError::NotFound(format!("plan {plan_id} not found")))?;
            if !plan.active {
                return Err(AppError::BadRequest("admin.error.plan_inactive".to_string()));
            }
            let codes = crate::services::token::generate_tokens(
                &state.db,
                &state.config.token,
                plan_id,
                count,
                Some(token_name),
            ).await.map_err(AppError::Internal)?;
            state.db.audit_log(&state.config.admin.username, "generate_tokens", Some("token"), Some(plan_id), Some(&format!("{} tokens for plan {}", codes.len(), plan_id))).await.ok();
            Some(I18nMessage::with_args("admin.manage.tokens_generated", [
                ("count", codes.len().to_string()),
            ]))
        } else {
            None
        }
    } else {
        None
    };

    let (token_page, plans, sessions) = load_manage_data(&state, "current", 1).await?;
    let ctx = admin_ctx(&state).await;
    render(&ManageTemplate {
        token_page,
        plans,
        sessions,
        message,
        csrf_token: csrf.0,
        cafe_name: ctx.cafe_name,
        current_page: "manage".to_string(),
        theme_css: ctx.theme_css,
    })
}

// -- Shared helpers --

/// Validate plan input fields. Returns `Err(BadRequest)` on invalid data.
pub fn validate_plan_input(name: &str, duration: i64, price: i64) -> Result<(), AppError> {
    if name.is_empty() {
        return Err(AppError::BadRequest("admin.error.name_empty".to_string()));
    }
    if name.len() > 100 {
        return Err(AppError::BadRequest("admin.error.name_too_long".to_string()));
    }
    if duration <= 0 {
        return Err(AppError::BadRequest("admin.error.duration_positive".to_string()));
    }
    if duration > 1440 {
        return Err(AppError::BadRequest("admin.error.duration_max".to_string()));
    }
    if price < 0 {
        return Err(AppError::BadRequest("admin.error.price_negative".to_string()));
    }
    Ok(())
}

/// Parse and validate token query params from the manage page URL.
/// Returns (status_filter, page) with safe defaults.
///
/// The status filter is always `Some(...)`:
///   - `"current"` (default) → unused + active
///   - `"all"` → no filter
///   - `"unused"` / `"active"` / `"expired"` / `"revoked"` → single status
fn parse_token_query(query: &ManageQuery) -> (String, i64) {
    let status_filter = match query.token_status.as_deref() {
        Some("all") => "all".to_string(),
        Some("unused") => "unused".to_string(),
        Some("active") => "active".to_string(),
        Some("expired") => "expired".to_string(),
        Some("revoked") => "revoked".to_string(),
        _ => "current".to_string(), // default: unused + active
    };
    let page = query.token_page.unwrap_or(1).max(1);
    (status_filter, page)
}

/// Load all manage page data (token page, plans, sessions).
async fn load_manage_data(
    state: &Arc<AppState>,
    status_filter: &str,
    page: i64,
) -> Result<(crate::db::TokenPage, Vec<crate::db::Plan>, Vec<crate::db::Session>), AppError> {
    let db_filter = match status_filter {
        "all" => None,
        other => Some(other),
    };
    let (tokens, total) = state.db.list_tokens_paged(db_filter, page, TOKENS_PER_PAGE).await
        .map_err(AppError::Internal)?;
    let token_page = crate::db::TokenPage {
        tokens,
        total,
        page,
        per_page: TOKENS_PER_PAGE,
        status_filter: status_filter.to_string(),
    };
    let plans = state.db.list_plans().await.map_err(AppError::Internal)?;
    let sessions = state.db.get_active_sessions().await.map_err(AppError::Internal)?;
    Ok((token_page, plans, sessions))
}

/// GET /admin/audit — audit log viewer (requires auth)
async fn audit_page(
    State(state): State<Arc<AppState>>,
    _admin: AdminSession,
    Query(query): Query<AuditQuery>,
) -> Result<impl IntoResponse, AppError> {
    let page = query.page.unwrap_or(1).max(1);
    let (entries, total) = state.db.get_audit_log_paged(page, AUDIT_PER_PAGE).await
        .map_err(AppError::Internal)?;
    let audit_page = crate::db::AuditPage {
        entries,
        total,
        page,
        per_page: AUDIT_PER_PAGE,
    };
    let ctx = admin_ctx(&state).await;
    render(&AuditTemplate {
        audit_page,
        cafe_name: ctx.cafe_name,
        current_page: "audit".to_string(),
        theme_css: ctx.theme_css,
    })
}

/// GET /admin/settings — theme and contact settings (requires auth)
async fn settings_page(
    State(state): State<Arc<AppState>>,
    _admin: AdminSession,
    csrf: CsrfToken,
) -> Result<impl IntoResponse, AppError> {
    let current = portal_config_from_db(&state).await;
    let ctx = AdminCtx {
        cafe_name: current.cafe_name.clone(),
        theme_css: current.generate_theme_css(),
    };
    render(&SettingsTemplate {
        current,
        message: None,
        csrf_token: csrf.0,
        cafe_name: ctx.cafe_name,
        current_page: "settings".to_string(),
        theme_css: ctx.theme_css,
    })
}

/// POST /admin/settings — save branding and contact settings (requires auth)
async fn settings_submit(
    State(state): State<Arc<AppState>>,
    _admin: AdminSession,
    csrf: CsrfToken,
    Form(form): Form<SettingsForm>,
) -> Result<impl IntoResponse, AppError> {
    // CSRF validation (constant-time comparison)
    let csrf_ok: bool = form.csrf_token.as_bytes()
        .ct_eq(csrf.0.as_bytes())
        .into();
    if !csrf_ok {
        return Err(AppError::BadRequest("admin.error.csrf".to_string()));
    }

    // Validate inputs
    let cafe_name = form.cafe_name.trim();
    if cafe_name.is_empty() {
        return Err(AppError::BadRequest("admin.error.name_empty".to_string()));
    }
    if cafe_name.len() > 100 {
        return Err(AppError::BadRequest("admin.error.name_too_long".to_string()));
    }
    let welcome_message = form.welcome_message.trim();
    if welcome_message.len() > 500 {
        return Err(AppError::BadRequest("admin.error.welcome_too_long".to_string()));
    }
    let theme_color = form.theme_color.trim();
    if !crate::config::is_valid_hex_color(theme_color) {
        return Err(AppError::BadRequest("admin.error.invalid_color".to_string()));
    }
    let contact_name = form.contact_name.trim();
    if contact_name.len() > 100 {
        return Err(AppError::BadRequest("admin.error.name_too_long".to_string()));
    }
    let contact_phone = form.contact_phone.trim();
    if contact_phone.len() > 30 {
        return Err(AppError::BadRequest("admin.error.phone_too_long".to_string()));
    }
    let contact_hours = form.contact_hours.trim();
    if contact_hours.len() > 100 {
        return Err(AppError::BadRequest("admin.error.hours_too_long".to_string()));
    }

    // Persist all settings to DB
    state.db.set_setting("cafe_name", cafe_name).await.map_err(AppError::Internal)?;
    state.db.set_setting("welcome_message", welcome_message).await.map_err(AppError::Internal)?;
    state.db.set_setting("theme_color", theme_color).await.map_err(AppError::Internal)?;
    state.db.set_setting("contact_name", contact_name).await.map_err(AppError::Internal)?;
    state.db.set_setting("contact_phone", contact_phone).await.map_err(AppError::Internal)?;
    state.db.set_setting("contact_hours", contact_hours).await.map_err(AppError::Internal)?;

    // Audit log
    state.db.audit_log(
        &state.config.admin.username,
        "update_settings",
        Some("setting"),
        None,
        Some(&format!("cafe_name={cafe_name}, theme_color={theme_color}")),
    ).await.ok();

    // Re-read from DB and render the page with success message
    let current = portal_config_from_db(&state).await;
    let ctx = AdminCtx {
        cafe_name: current.cafe_name.clone(),
        theme_css: current.generate_theme_css(),
    };
    render(&SettingsTemplate {
        current,
        message: Some("admin.settings.saved".to_string()),
        csrf_token: csrf.0,
        cafe_name: ctx.cafe_name,
        current_page: "settings".to_string(),
        theme_css: ctx.theme_css,
    })
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
        let cookie = session_cookie("test-session-id", false);
        assert!(cookie.contains("didicafe_admin=test-session-id"));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Strict"));
        assert!(!cookie.contains("Secure"));
    }

    #[test]
    fn test_session_cookie_secure_flag() {
        let cookie = session_cookie("test-session-id", true);
        assert!(cookie.contains("didicafe_admin=test-session-id"));
        assert!(cookie.contains("Secure"));
    }

    #[test]
    fn test_clear_cookie_format() {
        let cookie = clear_session_cookie(false);
        assert!(cookie.contains("didicafe_admin="));
        assert!(cookie.contains("Max-Age=0"));
        assert!(cookie.contains("HttpOnly"));
        assert!(!cookie.contains("Secure"));
    }

    #[test]
    fn test_clear_cookie_secure_flag() {
        let cookie = clear_session_cookie(true);
        assert!(cookie.contains("Max-Age=0"));
        assert!(cookie.contains("Secure"));
    }
}
