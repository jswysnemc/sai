use super::super::super::app_state::WebAppState;
use super::super::super::error::{WebError, WebResult};
use crate::super_agent::cards;
use crate::super_agent::model::{GroupDetailSnapshot, GroupMessage, GroupStatus, SuperAgentOverview};
use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
pub(super) struct CreateGroupBody {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    workspace_path: String,
    leader_name: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct UpdateGroupBody {
    name: Option<String>,
    description: Option<String>,
    status: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct MessageQuery {
    after_seq: Option<i64>,
    limit: Option<usize>,
}

#[derive(Deserialize)]
pub(super) struct PostMessageBody {
    content: String,
    #[serde(default)]
    sender_name: String,
    thread_id: Option<String>,
    recipient_id: Option<String>,
    #[serde(default = "default_true")]
    auto_dispatch: bool,
}

#[derive(Deserialize)]
pub(super) struct RecallQuery {
    kind: String,
    object_id: String,
    #[serde(default)]
    offset: usize,
    #[serde(default = "default_recall_limit")]
    limit: usize,
}

fn default_true() -> bool {
    true
}

fn default_recall_limit() -> usize {
    4_000
}

/// 返回全局 Super-Agent 概览统计与群组列表（首次访问自动确保默认群组存在）。
pub(super) async fn overview(State(state): State<WebAppState>) -> WebResult<Json<SuperAgentOverview>> {
    let active_ws = state.workspaces.active().ok().map(|ws| ws.path);
    let _ = state
        .super_agent
        .ensure_default_group(active_ws.as_deref())
        .map_err(WebError::from)?;
    let data = state.super_agent.overview().map_err(WebError::from)?;
    Ok(Json(data))
}

/// 列出全部群组摘要。
pub(super) async fn list_groups(State(state): State<WebAppState>) -> WebResult<Json<Value>> {
    let data = state.super_agent.overview().map_err(WebError::from)?;
    Ok(Json(json!({ "groups": data.groups })))
}

/// 创建新 Super-Agent 协作群组。
pub(super) async fn create_group(
    State(state): State<WebAppState>,
    Json(body): Json<CreateGroupBody>,
) -> WebResult<Json<Value>> {
    let name = body.name.trim();
    if name.is_empty() {
        return Err(WebError::bad_request("group name is required"));
    }
    let ws = if body.workspace_path.trim().is_empty() {
        state
            .workspaces
            .active()
            .map(|info| info.path)
            .unwrap_or_default()
    } else {
        body.workspace_path.trim().to_string()
    };
    let (group, leader) = state
        .super_agent
        .create_group(name, body.description.trim(), &ws, body.leader_name.as_deref())
        .map_err(WebError::from)?;
    Ok(Json(json!({ "group": group, "leader": leader })))
}

/// 获取单个群组的完整详情快照。
pub(super) async fn group_detail(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
) -> WebResult<Json<GroupDetailSnapshot>> {
    let detail = state
        .super_agent
        .group_detail(&id, 150)
        .map_err(|err| WebError::not_found(err.to_string()))?;
    Ok(Json(detail))
}

/// 更新群组名称、描述或状态。
pub(super) async fn update_group(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
    Json(body): Json<UpdateGroupBody>,
) -> WebResult<Json<Value>> {
    let status = body.status.as_deref().map(GroupStatus::parse);
    let group = state
        .super_agent
        .update_group(&id, body.name.as_deref(), body.description.as_deref(), status)
        .map_err(|err| WebError::bad_request(err.to_string()))?;
    Ok(Json(json!({ "group": group })))
}

/// 删除指定群组。
pub(super) async fn delete_group(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
) -> WebResult<Json<Value>> {
    let deleted = state
        .super_agent
        .delete_group(&id)
        .map_err(WebError::from)?;
    Ok(Json(json!({ "deleted": deleted })))
}

/// 按增量序号查询群组消息列表。
pub(super) async fn list_messages(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
    Query(query): Query<MessageQuery>,
) -> WebResult<Json<Vec<GroupMessage>>> {
    let limit = query.limit.unwrap_or(120).clamp(1, 500);
    let messages = state
        .super_agent
        .store()
        .list_messages(&id, limit, query.after_seq)
        .map_err(WebError::from)?;
    Ok(Json(messages))
}

/// 在群组中发送一条用户消息并触发两级路由与 Leader/Runner 异步执行。
pub(super) async fn post_message(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
    Json(body): Json<PostMessageBody>,
) -> WebResult<Json<Value>> {
    let content = body.content.trim();
    if content.is_empty() {
        return Err(WebError::bad_request("message content is required"));
    }
    let sender = if body.sender_name.trim().is_empty() {
        "You"
    } else {
        body.sender_name.trim()
    };
    let (message, decision) = state
        .super_agent
        .post_user_message(
            &id,
            sender,
            content,
            body.thread_id.as_deref(),
            body.recipient_id.as_deref(),
            "web",
            body.auto_dispatch,
        )
        .await
        .map_err(|err| WebError::bad_request(err.to_string()))?;
    Ok(Json(json!({
        "message": message,
        "route_path": decision.route_path(),
    })))
}

/// 按 Chain of Recall 协议分页读取指定对象的完整详情。
pub(super) async fn recall(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
    Query(query): Query<RecallQuery>,
) -> WebResult<Json<Value>> {
    let data = cards::recall_object(
        state.super_agent.store(),
        &id,
        &query.kind,
        &query.object_id,
        query.offset,
        query.limit.clamp(256, 20_000),
    )
    .map_err(|err| WebError::not_found(err.to_string()))?;
    Ok(Json(data))
}
