use std::sync::Arc;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
};
use serde::{Deserialize, Serialize};

use crate::AppState;
use super::AppError;
use super::extractors::AdminSession;

// -- Consistent API response envelope --

/// Wraps all successful API responses in a consistent envelope.
#[derive(Serialize)]
struct ApiResponse<T: Serialize> {
    ok: bool,
    data: T,
}

impl<T: Serialize> ApiResponse<T> {
    fn success(data: T) -> Json<ApiResponse<T>> {
        Json(ApiResponse { ok: true, data })
    }
}

// -- Request types --

#[derive(Deserialize)]
pub struct CreatePlanRequest {
    name: String,
    duration_minutes: i64,
    price_ariary: i64,
}

#[derive(Deserialize)]
pub struct UpdatePlanRequest {
    name: String,
    duration_minutes: i64,
    price_ariary: i64,
    active: bool,
}

#[derive(Deserialize)]
pub struct GenerateTokensRequest {
    plan_id: i64,
    count: usize,
    name: String,
}

#[derive(Deserialize)]
pub struct TokensQuery {
    status: Option<String>,
}

// -- Routes --

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/plans", get(list_plans).post(create_plan))
        .route("/api/plans/{id}", put(update_plan))
        .route("/api/tokens", get(list_tokens))
        .route("/api/tokens/generate", post(generate_tokens))
        .route("/api/tokens/{id}", delete(revoke_token))
        .route("/api/sessions", get(list_sessions))
        .route("/api/sessions/{id}", delete(disconnect_session))
        .route("/api/stats", get(get_stats))
}

// -- Handlers --

async fn list_plans(
    State(state): State<Arc<AppState>>,
    _admin: AdminSession,
) -> Result<impl IntoResponse, AppError> {
    let plans = state.db.list_plans().await
        .map_err(AppError::Internal)?;
    Ok(ApiResponse::success(serde_json::json!({ "plans": plans })))
}

async fn create_plan(
    State(state): State<Arc<AppState>>,
    _admin: AdminSession,
    Json(req): Json<CreatePlanRequest>,
) -> Result<impl IntoResponse, AppError> {
    if req.name.is_empty() {
        return Err(AppError::BadRequest("name must not be empty".to_string()));
    }
    if req.duration_minutes <= 0 {
        return Err(AppError::BadRequest("duration_minutes must be > 0".to_string()));
    }
    if req.price_ariary < 0 {
        return Err(AppError::BadRequest("price_ariary must be >= 0".to_string()));
    }

    let id = state.db.create_plan(&req.name, req.duration_minutes, req.price_ariary).await
        .map_err(AppError::Internal)?;
    Ok((StatusCode::CREATED, ApiResponse::success(serde_json::json!({ "id": id }))))
}

async fn update_plan(
    State(state): State<Arc<AppState>>,
    _admin: AdminSession,
    Path(id): Path<i64>,
    Json(req): Json<UpdatePlanRequest>,
) -> Result<impl IntoResponse, AppError> {
    if req.name.is_empty() {
        return Err(AppError::BadRequest("name must not be empty".to_string()));
    }
    if req.duration_minutes <= 0 {
        return Err(AppError::BadRequest("duration_minutes must be > 0".to_string()));
    }
    if req.price_ariary < 0 {
        return Err(AppError::BadRequest("price_ariary must be >= 0".to_string()));
    }

    let updated = state.db.update_plan(id, &req.name, req.duration_minutes, req.price_ariary, req.active).await
        .map_err(AppError::Internal)?;
    if !updated {
        return Err(AppError::NotFound(format!("plan {id} not found")));
    }
    Ok(ApiResponse::success(serde_json::json!({ "id": id })))
}

async fn list_tokens(
    State(state): State<Arc<AppState>>,
    _admin: AdminSession,
    Query(query): Query<TokensQuery>,
) -> Result<impl IntoResponse, AppError> {
    // Validate status filter if provided
    if let Some(ref status) = query.status
        && !crate::db::TokenStatus::ALL.contains(&status.as_str())
    {
        return Err(AppError::BadRequest(format!(
            "invalid status filter '{}', must be one of: {}",
            status,
            crate::db::TokenStatus::ALL.join(", ")
        )));
    }

    let tokens = state.db.list_tokens(query.status.as_deref()).await
        .map_err(AppError::Internal)?;
    Ok(ApiResponse::success(serde_json::json!({ "tokens": tokens })))
}

async fn generate_tokens(
    State(state): State<Arc<AppState>>,
    _admin: AdminSession,
    Json(req): Json<GenerateTokensRequest>,
) -> Result<impl IntoResponse, AppError> {
    if req.name.is_empty() {
        return Err(AppError::BadRequest("name must not be empty".to_string()));
    }
    if req.count == 0 {
        return Err(AppError::BadRequest("count must be > 0".to_string()));
    }
    if req.count > 100 {
        return Err(AppError::BadRequest("count must be <= 100".to_string()));
    }

    // Verify plan exists
    let plan = state.db.get_plan(req.plan_id).await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound(format!("plan {} not found", req.plan_id)))?;
    if !plan.active {
        return Err(AppError::BadRequest(format!("plan '{}' is not active", plan.name)));
    }

    let codes = crate::services::token::generate_tokens(
        &state.db,
        &state.config.token,
        req.plan_id,
        req.count,
        Some(&req.name),
    ).await.map_err(AppError::Internal)?;
    Ok((StatusCode::CREATED, ApiResponse::success(serde_json::json!({ "tokens": codes }))))
}

async fn revoke_token(
    State(state): State<Arc<AppState>>,
    _admin: AdminSession,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    // Verify the token exists before attempting revocation
    let token = state.db.get_token_by_id(id).await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound(format!("token {id} not found")))?;

    if token.token_status() != Some(crate::db::TokenStatus::Unused) {
        return Err(AppError::BadRequest(format!(
            "cannot revoke token with status '{}'", token.status
        )));
    }

    state.db.revoke_token(id).await
        .map_err(AppError::Internal)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_sessions(
    State(state): State<Arc<AppState>>,
    _admin: AdminSession,
) -> Result<impl IntoResponse, AppError> {
    let sessions = state.db.get_active_sessions().await
        .map_err(AppError::Internal)?;
    Ok(ApiResponse::success(serde_json::json!({ "sessions": sessions })))
}

async fn disconnect_session(
    State(state): State<Arc<AppState>>,
    _admin: AdminSession,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    crate::services::session::disconnect(&state, id).await
        .map_err(AppError::Firewall)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn get_stats(
    State(state): State<Arc<AppState>>,
    _admin: AdminSession,
) -> Result<impl IntoResponse, AppError> {
    let stats = state.db.get_daily_stats().await
        .map_err(AppError::Internal)?;
    Ok(ApiResponse::success(stats))
}
