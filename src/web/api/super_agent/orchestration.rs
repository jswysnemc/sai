use super::super::super::app_state::WebAppState;
use super::super::super::error::{WebError, WebResult};
use crate::super_agent::model::{SubscriptionMode, SuperTask, TriggerEvent};
use crate::super_agent::SpawnRunnerRequest;
use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
pub(super) struct SpawnRunnerBody {
    name: String,
    #[serde(default)]
    specialty: String,
    #[serde(default = "default_subagent_type")]
    subagent_type: String,
    #[serde(default)]
    task_title: String,
    prompt: String,
    thread_id: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct MessageRunnerBody {
    message: String,
    #[serde(default)]
    sender_name: String,
}

#[derive(Deserialize)]
pub(super) struct StopRunnerBody {
    #[serde(default)]
    reason: String,
}

#[derive(Deserialize)]
pub(super) struct SubscribeBody {
    action: String,
    thread_id: String,
    #[serde(default = "default_sub_mode")]
    mode: String,
}

fn default_subagent_type() -> String {
    "general".to_string()
}

fn default_sub_mode() -> String {
    "owner".to_string()
}

/// 在群组中新建并异步启动 Worker(Runner)。
pub(super) async fn spawn_runner(
    State(state): State<WebAppState>,
    Path(group_id): Path<String>,
    Json(body): Json<SpawnRunnerBody>,
) -> WebResult<Json<Value>> {
    let name = body.name.trim();
    let prompt = body.prompt.trim();
    if name.is_empty() || prompt.is_empty() {
        return Err(WebError::bad_request("runner name and prompt are required"));
    }
    let task_title = if body.task_title.trim().is_empty() {
        prompt.chars().take(48).collect()
    } else {
        body.task_title.trim().to_string()
    };
    let (member, task) = state
        .super_agent
        .spawn_runner(SpawnRunnerRequest {
            group_id,
            name: name.to_string(),
            specialty: body.specialty.trim().to_string(),
            subagent_type: body.subagent_type.trim().to_string(),
            task_title,
            prompt: prompt.to_string(),
            thread_id: body.thread_id,
            origin_channel: Some("web".to_string()),
            origin_target: None,
        })
        .await
        .map_err(|err| WebError::bad_request(err.to_string()))?;
    Ok(Json(json!({ "member": member, "task": task })))
}

/// 向指定 Runner 发送追问或协作指令。
pub(super) async fn message_runner(
    State(state): State<WebAppState>,
    Path((group_id, runner_id)): Path<(String, String)>,
    Json(body): Json<MessageRunnerBody>,
) -> WebResult<Json<Value>> {
    let message = body.message.trim();
    if message.is_empty() {
        return Err(WebError::bad_request("message is required"));
    }
    let sender = if body.sender_name.trim().is_empty() {
        "You"
    } else {
        body.sender_name.trim()
    };
    state
        .super_agent
        .message_runner(&group_id, &runner_id, sender, message)
        .await
        .map_err(|err| WebError::bad_request(err.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// 停止指定 Runner。
pub(super) async fn stop_runner(
    State(state): State<WebAppState>,
    Path((group_id, runner_id)): Path<(String, String)>,
    body: Option<Json<StopRunnerBody>>,
) -> WebResult<Json<Value>> {
    let reason = body
        .map(|Json(b)| b.reason)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "stopped from WebUI".to_string());
    let member = state
        .super_agent
        .stop_runner(&group_id, &runner_id, &reason)
        .map_err(|err| WebError::bad_request(err.to_string()))?;
    Ok(Json(json!({ "member": member })))
}

/// 为 Runner 订阅或取消订阅指定 Thread（多对多订阅直通路由）。
pub(super) async fn update_subscription(
    State(state): State<WebAppState>,
    Path((group_id, runner_id)): Path<(String, String)>,
    Json(body): Json<SubscribeBody>,
) -> WebResult<Json<Value>> {
    match body.action.trim().to_ascii_lowercase().as_str() {
        "subscribe" => {
            let sub = state
                .super_agent
                .subscribe_thread(
                    &group_id,
                    body.thread_id.trim(),
                    &runner_id,
                    SubscriptionMode::parse(&body.mode),
                )
                .map_err(|err| WebError::bad_request(err.to_string()))?;
            Ok(Json(json!({ "ok": true, "subscription": sub })))
        }
        "unsubscribe" => {
            let removed = state
                .super_agent
                .unsubscribe_thread(&group_id, body.thread_id.trim(), &runner_id)
                .map_err(|err| WebError::bad_request(err.to_string()))?;
            Ok(Json(json!({ "ok": true, "removed": removed })))
        }
        other => Err(WebError::bad_request(format!(
            "unsupported subscription action: {other}"
        ))),
    }
}

/// 列出群组任务与关联 Runner 报告。
pub(super) async fn list_tasks(
    State(state): State<WebAppState>,
    Path(group_id): Path<String>,
) -> WebResult<Json<Value>> {
    let tasks: Vec<SuperTask> = state
        .super_agent
        .store()
        .list_tasks(&group_id)
        .map_err(WebError::from)?;
    let reports = state
        .super_agent
        .store()
        .list_reports(&group_id, 60)
        .map_err(WebError::from)?;
    Ok(Json(json!({ "tasks": tasks, "reports": reports })))
}

/// 重试任务。
pub(super) async fn retry_task(
    State(state): State<WebAppState>,
    Path((group_id, task_id)): Path<(String, String)>,
) -> WebResult<Json<Value>> {
    let task = state
        .super_agent
        .retry_task(&group_id, &task_id)
        .await
        .map_err(|err| WebError::bad_request(err.to_string()))?;
    Ok(Json(json!({ "task": task })))
}

/// 停止任务及其关联 Runner。
pub(super) async fn stop_task(
    State(state): State<WebAppState>,
    Path((group_id, task_id)): Path<(String, String)>,
) -> WebResult<Json<Value>> {
    let task = state
        .super_agent
        .stop_task(&group_id, &task_id)
        .map_err(|err| WebError::bad_request(err.to_string()))?;
    Ok(Json(json!({ "task": task })))
}

/// 列出群组两级过滤 Trigger 审计记录。
pub(super) async fn list_triggers(
    State(state): State<WebAppState>,
    Path(group_id): Path<String>,
) -> WebResult<Json<Vec<TriggerEvent>>> {
    let triggers = state
        .super_agent
        .store()
        .list_triggers(&group_id, 80)
        .map_err(WebError::from)?;
    Ok(Json(triggers))
}
