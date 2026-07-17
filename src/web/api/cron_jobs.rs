use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use crate::cron::{CronJob, CronRepository};
use axum::extract::{Path, State};
use axum::routing::{get, patch};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct CreateCronJobRequest {
    name: String,
    prompt: String,
    session_id: String,
    run_at: i64,
    interval_seconds: Option<i64>,
}

#[derive(Deserialize)]
struct UpdateCronJobRequest {
    enabled: bool,
}

#[derive(Serialize)]
struct DeleteCronJobResponse {
    removed: bool,
}

/// 返回定时任务管理路由。
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/cron-jobs", get(list).post(create))
        .route("/api/cron-jobs/:id", patch(update).delete(remove))
}

/// 列出全部持久化定时任务。
async fn list(State(state): State<WebAppState>) -> WebResult<Json<Vec<CronJob>>> {
    Ok(Json(
        CronRepository::new(&state.paths)
            .and_then(|repository| repository.list())
            .map_err(WebError::from)?,
    ))
}

/// 创建持久化定时任务。
async fn create(
    State(state): State<WebAppState>,
    Json(request): Json<CreateCronJobRequest>,
) -> WebResult<Json<CronJob>> {
    let repository = CronRepository::new(&state.paths).map_err(WebError::from)?;
    Ok(Json(
        repository
            .create(
                &request.name,
                &request.prompt,
                &request.session_id,
                request.run_at,
                request.interval_seconds,
            )
            .map_err(|error| WebError::bad_request(error.to_string()))?,
    ))
}

/// 启用或停用持久化定时任务。
async fn update(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
    Json(request): Json<UpdateCronJobRequest>,
) -> WebResult<Json<CronJob>> {
    Ok(Json(
        CronRepository::new(&state.paths)
            .and_then(|repository| repository.set_enabled(&id, request.enabled))
            .map_err(|error| WebError::bad_request(error.to_string()))?,
    ))
}

/// 删除持久化定时任务。
async fn remove(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
) -> WebResult<Json<DeleteCronJobResponse>> {
    Ok(Json(DeleteCronJobResponse {
        removed: CronRepository::new(&state.paths)
            .and_then(|repository| repository.remove(&id))
            .map_err(WebError::from)?,
    }))
}
