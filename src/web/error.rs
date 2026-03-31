use askama::Template;
use axum::{
    Json,
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use serde_json::json;
use tracing::error;

/// Application-level error type for all handlers.
///
/// Maps domain errors to appropriate HTTP status codes.
/// Internal details are logged server-side but never leaked to clients.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("firewall error: {0}")]
    Firewall(anyhow::Error),

    #[error("authentication required")]
    Unauthorized {
        /// If true, return 401 JSON (for API endpoints). If false, redirect to login.
        api_path: bool,
    },

    #[error("not found: {0}")]
    NotFound(String),

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("rate limited")]
    RateLimited {
        /// Seconds until the client should retry (for `Retry-After` header).
        retry_after: Option<u64>,
    },

    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, client_message) = match &self {
            AppError::Database(e) => {
                error!("database error: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error.")
            }
            AppError::Firewall(e) => {
                error!("firewall error: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error.")
            }
            AppError::Unauthorized { api_path } => {
                if *api_path {
                    return (
                        StatusCode::UNAUTHORIZED,
                        Json(json!({ "error": "authentication required" })),
                    ).into_response();
                }
                return Redirect::to("/admin/login").into_response();
            }
            AppError::NotFound(detail) => {
                // NotFound details are safe to expose (e.g. "token not found")
                return (StatusCode::NOT_FOUND, detail.clone()).into_response();
            }
            AppError::BadRequest(detail) => {
                // BadRequest details are safe to expose (e.g. "invalid token format")
                return (StatusCode::BAD_REQUEST, detail.clone()).into_response();
            }
            AppError::RateLimited { retry_after } => {
                // 429 Too Many Requests — never leak whether a token exists (OWASP)
                let mut response = (
                    StatusCode::TOO_MANY_REQUESTS,
                    "Too many attempts. Please wait and try again.",
                )
                    .into_response();
                if let Some(secs) = retry_after
                    && let Ok(val) = axum::http::HeaderValue::from_str(&secs.to_string())
                {
                    response.headers_mut().insert("Retry-After", val);
                }
                return response;
            }
            AppError::Internal(e) => {
                error!("internal error: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error.")
            }
        };

        (status, client_message.to_string()).into_response()
    }
}

/// Render an askama template into an HTML response.
///
/// askama 0.15 removed the built-in axum integration (askama_axum is gone).
/// This helper renders the template and wraps it in `Html<String>`.
pub fn render<T: Template>(tpl: &T) -> Result<Html<String>, AppError> {
    tpl.render()
        .map(Html)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("template render error: {e}")))
}
