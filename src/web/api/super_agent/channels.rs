use super::super::super::app_state::WebAppState;
use super::super::super::error::{WebError, WebResult};
use crate::config::{AppConfig, SuperAgentConfig};
use crate::super_agent::channel_bridge::{default_channel_target, ChannelInboundMessage};
use crate::super_agent::model::{ChannelBinding, WatchdogEvent};
use axum::extract::{Path, Query, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::Json;
use futures_util::stream::{Stream, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use std::convert::Infallible;
use std::time::Duration;
use tokio_stream::wrappers::BroadcastStream;

#[derive(Deserialize)]
pub(super) struct BindChannelBody {
    channel: String,
    #[serde(default = "default_wildcard")]
    external_chat_id: String,
    #[serde(default)]
    label: String,
    #[serde(default = "default_admission")]
    admission_mode: String,
    #[serde(default = "default_true")]
    enabled: bool,
}

#[derive(Deserialize)]
pub(super) struct ChannelInboundBody {
    channel: String,
    external_chat_id: String,
    #[serde(default)]
    actor: String,
    content: String,
    #[serde(default = "default_true")]
    simulate_outbound: bool,
    #[serde(default = "default_true")]
    auto_dispatch: bool,
}

#[derive(Deserialize)]
pub(super) struct StreamQuery {
    group_id: Option<String>,
}

fn default_wildcard() -> String {
    "*".to_string()
}

fn default_admission() -> String {
    "auto".to_string()
}

fn default_true() -> bool {
    true
}

/// 列出指定群组绑定的全部渠道。
pub(super) async fn list_channels(
    State(state): State<WebAppState>,
    Path(group_id): Path<String>,
) -> WebResult<Json<Vec<ChannelBinding>>> {
    let items = state
        .super_agent
        .store()
        .list_channel_bindings(Some(&group_id))
        .map_err(WebError::from)?;
    Ok(Json(items))
}

/// 绑定外部渠道（微信 / 飞书 / QQ / OneBot）到指定群组。
pub(super) async fn bind_channel(
    State(state): State<WebAppState>,
    Path(group_id): Path<String>,
    Json(body): Json<BindChannelBody>,
) -> WebResult<Json<ChannelBinding>> {
    let channel = body.channel.trim().to_ascii_lowercase();
    if channel.is_empty() {
        return Err(WebError::bad_request("channel is required"));
    }
    let chat_id = if body.external_chat_id.trim().is_empty() {
        "*"
    } else {
        body.external_chat_id.trim()
    };
    let label = if body.label.trim().is_empty() {
        format!("{channel}:{chat_id}")
    } else {
        body.label.trim().to_string()
    };
    let binding = state
        .super_agent
        .bind_channel(
            &group_id,
            &channel,
            chat_id,
            &label,
            body.admission_mode.trim(),
            body.enabled,
        )
        .map_err(|err| WebError::bad_request(err.to_string()))?;
    Ok(Json(binding))
}

/// 解绑外部渠道。
pub(super) async fn unbind_channel(
    State(state): State<WebAppState>,
    Path((group_id, binding_id)): Path<(String, String)>,
) -> WebResult<Json<Value>> {
    let removed = state
        .super_agent
        .unbind_channel(&group_id, &binding_id)
        .map_err(WebError::from)?;
    Ok(Json(json!({ "removed": removed })))
}

/// 接收一条外部渠道（或模拟渠道）消息并交给 Leader 编排处理。
pub(super) async fn ingest_channel_inbound(
    State(state): State<WebAppState>,
    Path(group_id): Path<String>,
    Json(body): Json<ChannelInboundBody>,
) -> WebResult<Json<Value>> {
    let channel = body.channel.trim().to_ascii_lowercase();
    let content = body.content.trim();
    if channel.is_empty() || content.is_empty() {
        return Err(WebError::bad_request("channel and content are required"));
    }
    let chat_id = if body.external_chat_id.trim().is_empty() {
        "default"
    } else {
        body.external_chat_id.trim()
    };
    let actor = if body.actor.trim().is_empty() {
        format!("{channel}_user")
    } else {
        body.actor.trim().to_string()
    };
    let (routed_group_id, message, decision) = state
        .super_agent
        .ingest_channel_message(
            ChannelInboundMessage {
                channel,
                external_chat_id: chat_id.to_string(),
                actor,
                content: content.to_string(),
                group_id: Some(group_id),
                reply_target: default_channel_target(chat_id, body.simulate_outbound),
                simulate_outbound: body.simulate_outbound,
            },
            body.auto_dispatch,
        )
        .await
        .map_err(|err| WebError::bad_request(err.to_string()))?;
    Ok(Json(json!({
        "group_id": routed_group_id,
        "message": message,
        "route_path": decision.route_path(),
    })))
}

/// 列出最近 Watchdog 外部守护事件。
pub(super) async fn list_watchdog(
    State(state): State<WebAppState>,
) -> WebResult<Json<Vec<WatchdogEvent>>> {
    let events = state
        .super_agent
        .store()
        .list_watchdog_events(60)
        .map_err(WebError::from)?;
    Ok(Json(events))
}

/// 立即执行一次 Watchdog 健康巡检。
pub(super) async fn run_watchdog_check(
    State(state): State<WebAppState>,
) -> WebResult<Json<Vec<WatchdogEvent>>> {
    let events = state
        .super_agent
        .run_watchdog_check()
        .await
        .map_err(WebError::from)?;
    Ok(Json(events))
}

/// 读取暴露给 Leader / Runner 的工具架构设计及本地网关（微信/QQ/飞书）实时状态。
pub(super) async fn capabilities(State(state): State<WebAppState>) -> WebResult<Json<Value>> {
    let config = AppConfig::load_or_default(&state.paths).map_err(WebError::from)?;
    let records = crate::gateways::process_control::refresh_gateway_processes(&state.paths)
        .unwrap_or_default();
    let is_running = |id: &str| {
        records
            .iter()
            .any(|record| record.gateway_id == id && record.is_running())
    };
    let weixin_logged_in = !config.gateways.weixin.token.trim().is_empty()
        || crate::gateways::weixin_bot::login::load_weixin_account(
            &state.paths,
            (!config.gateways.weixin.account.trim().is_empty())
                .then_some(config.gateways.weixin.account.trim()),
        )
        .is_ok();
    let qq_configured = !config.gateways.qq.app_id.trim().is_empty()
        || !config.gateways.qq.token.trim().is_empty();
    let feishu_configured = !config.gateways.feishu.app_id.trim().is_empty()
        && !config.gateways.feishu.app_secret.trim().is_empty();

    let leader_tools: Vec<Value> = crate::super_agent::tools::leader_tool_design()
        .iter()
        .map(|doc| {
            json!({
                "name": doc.name,
                "category": doc.category,
                "summary": doc.summary,
            })
        })
        .collect();
    let worker_tools: Vec<Value> = crate::super_agent::tools::worker_tool_design()
        .iter()
        .map(|doc| {
            json!({
                "name": doc.name,
                "category": doc.category,
                "summary": doc.summary,
            })
        })
        .collect();

    Ok(Json(json!({
        "leader_tools": leader_tools,
        "leader_readonly_tools": crate::super_agent::tools::LEADER_READONLY_TOOLS,
        "leader_excluded_tools": [
            "run_command",
            "background_command",
            "write_file",
            "str_replace",
            "apply_patch",
            "trash_path",
            "browser_*",
            "computer_*",
            "ssh_*",
            "subagent"
        ],
        "worker_tools": worker_tools,
        "gateways": {
            "channel_leader_routing": config.super_agent.channel_leader_routing,
            "weixin": {
                "logged_in": weixin_logged_in,
                "running": is_running("weixin"),
                "enabled": config.gateways.weixin.enabled,
            },
            "qq": {
                "configured": qq_configured,
                "running": is_running("qq"),
                "enabled": config.gateways.qq.enabled,
            },
            "feishu": {
                "configured": feishu_configured,
                "enabled": config.gateways.feishu.enabled,
            }
        }
    })))
}

/// 读取当前 Super-Agent 配置。
pub(super) async fn get_config(
    State(state): State<WebAppState>,
) -> WebResult<Json<SuperAgentConfig>> {
    let config = AppConfig::load_or_default(&state.paths).map_err(WebError::from)?;
    Ok(Json(config.super_agent))
}

/// 更新并持久化 Super-Agent 配置（含双层异构模型、并发上限与渠道自动路由开关）。
pub(super) async fn update_config(
    State(state): State<WebAppState>,
    Json(next): Json<SuperAgentConfig>,
) -> WebResult<Json<SuperAgentConfig>> {
    next.validate()
        .map_err(|err| WebError::bad_request(err.to_string()))?;
    let mut config = AppConfig::load_or_default(&state.paths).map_err(WebError::from)?;
    config.super_agent = next.clone();
    config.save(&state.paths).map_err(WebError::from)?;
    state.super_agent.emit_event(
        "config.updated",
        None,
        serde_json::to_value(&next).unwrap_or_else(|_| json!({})),
    );
    Ok(Json(next))
}

/// 实时订阅 Super-Agent 全局或指定群组的 SSE 事件流。
pub(super) async fn stream(
    State(state): State<WebAppState>,
    Query(query): Query<StreamQuery>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let receiver = state.super_agent.subscribe_events();
    let filter_gid = query.group_id;
    let live = BroadcastStream::new(receiver).filter_map(move |item| {
        let matched = item.ok().filter(|ev| match (&filter_gid, &ev.group_id) {
            (Some(want), Some(actual)) => want == actual,
            (Some(_), None) => true,
            (None, _) => true,
        });
        async move {
            matched.map(|ev| {
                let data = serde_json::to_string(&ev).unwrap_or_else(|_| "{}".to_string());
                Ok::<_, Infallible>(
                    Event::default()
                        .id(ev.seq.to_string())
                        .event(ev.kind)
                        .data(data),
                )
            })
        }
    });
    Sse::new(live).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}
