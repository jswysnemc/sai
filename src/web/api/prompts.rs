use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use super::super::services::prompt_service;
use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct SavePromptRequest {
    name: String,
    content: String,
}

#[derive(Serialize)]
struct PromptListResponse {
    items: Vec<prompt_service::PromptSummary>,
}

#[derive(Serialize)]
struct DeletePromptResponse {
    removed: bool,
}

/// 返回提示词管理路由。
///
/// 返回:
/// - 提示词 API 路由
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/prompts/:kind", get(list).post(create))
        .route(
            "/api/prompts/:kind/:name",
            get(read).put(update).delete(remove),
        )
}

/// 列出指定类型的提示词。
async fn list(
    State(state): State<WebAppState>,
    Path(kind): Path<String>,
) -> WebResult<Json<PromptListResponse>> {
    let kind = prompt_service::parse_kind(&kind)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    let items = prompt_service::list(&state.paths, kind).map_err(WebError::from)?;
    Ok(Json(PromptListResponse { items }))
}

/// 读取单个提示词。
async fn read(
    State(state): State<WebAppState>,
    Path((kind, name)): Path<(String, String)>,
) -> WebResult<Json<prompt_service::PromptDocument>> {
    let kind = prompt_service::parse_kind(&kind)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    let document = prompt_service::read(&state.paths, kind, &name)
        .map_err(|error| WebError::not_found(error.to_string()))?;
    Ok(Json(document))
}

/// 创建提示词。
async fn create(
    State(state): State<WebAppState>,
    Path(kind): Path<String>,
    Json(request): Json<SavePromptRequest>,
) -> WebResult<Json<prompt_service::PromptDocument>> {
    let kind = prompt_service::parse_kind(&kind)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    let document = prompt_service::save(&state.paths, kind, None, &request.name, &request.content)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(document))
}

/// 更新或重命名提示词。
async fn update(
    State(state): State<WebAppState>,
    Path((kind, current_name)): Path<(String, String)>,
    Json(request): Json<SavePromptRequest>,
) -> WebResult<Json<prompt_service::PromptDocument>> {
    let kind = prompt_service::parse_kind(&kind)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    let document = prompt_service::save(
        &state.paths,
        kind,
        Some(&current_name),
        &request.name,
        &request.content,
    )
    .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(document))
}

/// 删除提示词。
async fn remove(
    State(state): State<WebAppState>,
    Path((kind, name)): Path<(String, String)>,
) -> WebResult<Json<DeletePromptResponse>> {
    let kind = prompt_service::parse_kind(&kind)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    let removed = prompt_service::remove(&state.paths, kind, &name)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(DeletePromptResponse { removed }))
}
