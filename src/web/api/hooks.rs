//! 受保护的 hooks 来源预览、摘要授权和最近执行记录。
use super::super::{
    app_state::WebAppState,
    error::{WebError, WebResult},
};
use axum::{
    extract::{Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

#[derive(Default, Deserialize)]
struct HookQuery {
    session_id: Option<String>,
    workspace_id: Option<String>,
}
#[derive(Deserialize)]
struct TrustRequest {
    source: String,
    digest: String,
    workspace_id: Option<String>,
}

/// 【Hooks】【管理路由】无参数，返回来源与记录接口，不提供未经审批的测试执行接口
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/hooks/sources", get(sources))
        .route("/api/hooks/trust", post(trust))
        .route("/api/hooks/history", get(history))
}

/// 【Hooks】【来源查询】参数为应用状态与工作区，返回定义和审查摘要
async fn sources(
    State(state): State<WebAppState>,
    Query(query): Query<HookQuery>,
) -> WebResult<Json<Vec<crate::hooks::sources::SourcePreview>>> {
    let workspace = match query.workspace_id.as_deref() {
        Some(id) => state.workspaces.get(id),
        None => state.workspaces.active(),
    }
    .map_err(WebError::from)?;
    let config = crate::config::AppConfig::load_or_default(&state.paths).map_err(WebError::from)?;
    Ok(Json(
        crate::hooks::sources::preview(
            &config,
            &state.paths,
            std::path::Path::new(&workspace.path),
        )
        .map_err(WebError::from)?,
    ))
}

/// 【Hooks】【来源授权】参数为应用状态、来源和用户确认摘要，返回保存结果，变化后必须重新审查
async fn trust(
    State(state): State<WebAppState>,
    Json(request): Json<TrustRequest>,
) -> WebResult<Json<serde_json::Value>> {
    let Json(sources) = sources(
        State(state.clone()),
        Query(HookQuery {
            workspace_id: request.workspace_id,
            ..Default::default()
        }),
    )
    .await?;
    let source = sources
        .iter()
        .find(|source| source.path == request.source)
        .ok_or_else(|| WebError::bad_request("hook source is not active"))?;
    crate::hooks::trust::approve(&state.paths, source, &request.digest).map_err(WebError::from)?;
    Ok(Json(serde_json::json!({"trusted":true})))
}

/// 【Hooks】【记录查询】参数为应用状态和可选会话，返回有界结果列表
async fn history(
    State(state): State<WebAppState>,
    Query(query): Query<HookQuery>,
) -> Json<Vec<crate::hooks::history::HookRecord>> {
    Json(crate::hooks::history::recent(
        &state.paths,
        query.session_id.as_deref(),
    ))
}
