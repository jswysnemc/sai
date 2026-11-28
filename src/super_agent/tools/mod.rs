mod leader_tools;
mod worker_tools;

pub use leader_tools::register_leader_tools;
pub use worker_tools::register_worker_tools;

use super::cards;
use super::model::{MessageKind, MessageSenderKind, SubscriptionMode};
use super::service::SuperAgentService;
use super::store::NewGroupMessage;
use crate::config::AppConfig;
use crate::tools::{ToolRegistry, ToolSpec};
use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};
use std::sync::Arc;

/// Leader 专用编排工具名列表。
pub const LEADER_TOOL_NAMES: &[&str] = &[
    "super_list_state",
    "super_spawn_runner",
    "super_dispatch_task",
    "super_message_runner",
    "super_stop_runner",
    "super_group_post",
    "super_deliver_result",
    "super_subscribe_thread",
    "super_recall",
];

/// Worker(Runner) 专用协作与汇报工具名列表。
#[allow(dead_code)]
pub const WORKER_TOOL_NAMES: &[&str] = &[
    "super_report_submit",
    "super_group_post",
    "super_recall",
    "super_subscribe_thread",
];

/// Leader 保留的轻量只读感知工具（用于拆解任务前快速核实仓库与环境，不陷入耗时执行）。
pub const LEADER_READONLY_TOOLS: &[&str] = &["read_file", "glob", "grep", "check_os_info"];

/// 工具暴露设计说明项（供 TUI 检视栏与系统提示词共享单一真源）。
#[derive(Debug, Clone, Copy)]
pub struct SuperToolDoc {
    pub name: &'static str,
    pub category: &'static str,
    pub summary: &'static str,
}

/// 【超级智能体】【工具过滤】判断工具名是否属于应从 Leader 隔离的重执行/冲突工具。
///
/// 参数:
/// - `name`: 工具名称
///
/// 返回:
/// - 应从 Leader 工具表排除时返回 true
pub fn is_leader_excluded_tool(name: &str) -> bool {
    if matches!(
        name,
        "run_command"
            | "background_command"
            | "write_file"
            | "str_replace"
            | "apply_patch"
            | "edit_file"
            | "trash_path"
            | "subagent"
            | "codemode"
            | "generate_image"
            | "mcp_manager"
            | "computer_use"
            | "browser"
            | "ssh_list_hosts"
            | "ssh_run_command"
            | "ssh_upload_file"
            | "ssh_download_file"
    ) {
        return true;
    }
    crate::tools::browser::BROWSER_TOOL_NAMES.contains(&name)
        || crate::tools::computer_use::is_tool(name)
}

/// 返回暴露给 Leader 的 9 个编排工具设计清单。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 工具设计文档切片
pub fn leader_tool_design() -> &'static [SuperToolDoc] {
    &[
        SuperToolDoc {
            name: "super_list_state",
            category: "态势感知",
            summary: "查看群组成员、任务、待归纳报告、线程订阅与渠道绑定",
        },
        SuperToolDoc {
            name: "super_spawn_runner",
            category: "编排派发",
            summary: "异步创建隔离上下文(可选 worktree)的 Runner 并绑定任务",
        },
        SuperToolDoc {
            name: "super_dispatch_task",
            category: "编排派发",
            summary: "向已有 Runner 派发新任务，不存在时按名称自动新建",
        },
        SuperToolDoc {
            name: "super_message_runner",
            category: "协同控制",
            summary: "向运行中 Runner 步间注入追问，或唤醒空闲 Runner",
        },
        SuperToolDoc {
            name: "super_stop_runner",
            category: "协同控制",
            summary: "立即中止指定 Runner 执行并更新成员状态",
        },
        SuperToolDoc {
            name: "super_group_post",
            category: "群组协同",
            summary: "在群聊公开同步进度或 @指定成员 定向沟通",
        },
        SuperToolDoc {
            name: "super_deliver_result",
            category: "成果交付",
            summary: "归纳 Runner 报告正式交付用户，并自动回推来源渠道",
        },
        SuperToolDoc {
            name: "super_subscribe_thread",
            category: "路由订阅",
            summary: "管理 Runner 对话题线程的 owner/context 订阅直通",
        },
        SuperToolDoc {
            name: "super_recall",
            category: "链式召回",
            summary: "按 ID 分页回读被 Spill 截断的报告/任务/消息原文",
        },
    ]
}

/// 返回暴露给 Worker(Runner) 的 4 个协作汇报工具设计清单。
///
/// 参数:
/// - 无
///
/// 返回:
/// - Worker 工具设计文档切片
pub fn worker_tool_design() -> &'static [SuperToolDoc] {
    &[
        SuperToolDoc {
            name: "super_report_submit",
            category: "成果上报",
            summary: "提交结构化报告(ready/progress/waiting/blocked/failed)唤醒 Leader",
        },
        SuperToolDoc {
            name: "super_group_post",
            category: "群组协同",
            summary: "在群聊同步关键发现或 @其他成员 直接协作",
        },
        SuperToolDoc {
            name: "super_subscribe_thread",
            category: "路由订阅",
            summary: "调整自己对子话题线程的订阅关系",
        },
        SuperToolDoc {
            name: "super_recall",
            category: "链式召回",
            summary: "按 ID 按需召回关联任务、报告或原始触发消息详情",
        },
    ]
}

/// 按角色（Leader 或 Worker）解析运行期 AppConfig（应用论文 2.1/2.2 节双层异构模型配置）。
///
/// 参数:
/// - `base`: 当前应用配置
/// - `is_leader`: true 使用 Leader 覆盖模型，false 使用 Worker 覆盖模型
///
/// 返回:
/// - 应用模型覆盖后的独立配置副本
pub fn role_runtime_config(base: &AppConfig, is_leader: bool) -> AppConfig {
    let mut config = base.clone();
    // 1. 清除外部档案运行期覆盖，避免网关或会话工具白名单误裁掉 super_* 专用工具
    config.agent_runtime = None;
    let (provider_id, model) = if is_leader {
        (
            base.super_agent.leader_provider_id.trim(),
            base.super_agent.leader_model.trim(),
        )
    } else {
        (
            base.super_agent.worker_provider_id.trim(),
            base.super_agent.worker_model.trim(),
        )
    };
    // 2. 若显式配置了角色模型覆盖则切换到对应供应商与模型
    if !provider_id.is_empty() && !model.is_empty() {
        let _ = config.set_active_provider_model(provider_id, model);
    }
    config
}

/// 返回配置生效后的展示模型标签。
///
/// 参数:
/// - `config`: 角色运行期配置
///
/// 返回:
/// - `provider/model` 文本
pub fn model_display_label(config: &AppConfig) -> String {
    config
        .provider(None)
        .map(|provider| format!("{}/{}", provider.id, provider.default_model))
        .unwrap_or_else(|_| config.active_provider.clone())
}

/// 注册群组发言工具（Leader 与 Worker 共用）。
///
/// 参数:
/// - `registry`: 工具注册表
/// - `service`: Super-Agent 服务
/// - `group_id`: 所属群组标识
/// - `default_thread_id`: 默认线程标识
/// - `sender_kind`: 发送者角色
/// - `sender_id`: 发送者成员标识
/// - `sender_name`: 发送者展示名
///
/// 返回:
/// - 无
#[allow(clippy::too_many_arguments)]
pub(super) fn register_group_post_tool(
    registry: &mut ToolRegistry,
    service: Arc<SuperAgentService>,
    group_id: String,
    default_thread_id: String,
    sender_kind: MessageSenderKind,
    sender_id: String,
    sender_name: String,
) {
    registry.register(
        ToolSpec::new(
            "super_group_post",
            "Post a message into the group chat room so the user, Leader, and other Runners can see and coordinate.",
            json!({
                "type": "object",
                "properties": {
                    "content": { "type": "string", "description": "Message text to post in the group." },
                    "recipient": { "type": "string", "description": "Optional target member name or id to address (e.g. Leader or another runner)." },
                    "thread_id": { "type": "string", "description": "Optional thread id." },
                    "kind": { "type": "string", "enum": ["chat", "dispatch", "progress"], "description": "Optional message kind." }
                },
                "required": ["content"],
                "additionalProperties": false
            }),
            move |args| {
                let service = Arc::clone(&service);
                let group_id = group_id.clone();
                let default_thread_id = default_thread_id.clone();
                let sender_id = sender_id.clone();
                let sender_name = sender_name.clone();
                async move {
                    let content = required_str(&args, "content")?;
                    let thread_id = optional_str(&args, "thread_id")
                        .unwrap_or_else(|| default_thread_id.clone());
                    let recipient_ref = optional_str(&args, "recipient");
                    let (recipient_id, recipient_name) = match recipient_ref.as_deref() {
                        Some(target) => match service.store().find_member(&group_id, target)? {
                            Some(member) => (Some(member.id), Some(member.name)),
                            None => (None, Some(target.to_string())),
                        },
                        None => (None, None),
                    };
                    let kind = match optional_str(&args, "kind").as_deref() {
                        Some("dispatch") => MessageKind::Dispatch,
                        Some("progress") => MessageKind::Progress,
                        _ => MessageKind::Chat,
                    };
                    let message = service.record_group_message(NewGroupMessage {
                        group_id: group_id.clone(),
                        thread_id,
                        sender_kind,
                        sender_id,
                        sender_name: sender_name.clone(),
                        recipient_id: recipient_id.clone(),
                        recipient_name,
                        kind,
                        channel_source: None,
                        content: content.clone(),
                        summary: None,
                        related_task_id: None,
                        related_report_id: None,
                    })?;
                    // 若群组成员在发言中定向 @ 了另一个存活 Worker，同时直通投递到对方收件箱
                    if let Some(target_id) = recipient_id {
                        if let Ok(Some(target)) = service.store().get_member(&target_id) {
                            if target.role == super::model::MemberRole::Worker
                                && target.status.is_live()
                                && target.id != message.sender_id
                            {
                                let _ = service
                                    .deliver_to_runner_inbox(
                                        &group_id,
                                        &target.id,
                                        &sender_name,
                                        &content,
                                    )
                                    .await;
                            }
                        }
                    }
                    Ok(serde_json::to_string_pretty(&json!({
                        "ok": true,
                        "message_id": message.id,
                        "seq": message.seq,
                    }))?)
                }
            },
        )
        .writes(),
    );
}

/// 注册线程多对多订阅工具。
///
/// 参数:
/// - `registry`: 工具注册表
/// - `service`: Super-Agent 服务
/// - `group_id`: 所属群组标识
/// - `default_thread_id`: 默认线程标识
///
/// 返回:
/// - 无
pub(super) fn register_subscribe_tool(
    registry: &mut ToolRegistry,
    service: Arc<SuperAgentService>,
    group_id: String,
    default_thread_id: String,
) {
    registry.register(
        ToolSpec::new(
            "super_subscribe_thread",
            "Subscribe or unsubscribe a Runner to/from a Thread for direct fast-path event fan-out.",
            json!({
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["subscribe", "unsubscribe"] },
                    "runner": { "type": "string", "description": "Runner member_id or name." },
                    "thread_id": { "type": "string", "description": "Target thread id." },
                    "mode": { "type": "string", "enum": ["owner", "context"], "description": "Subscription mode (default owner)." }
                },
                "required": ["action", "runner"],
                "additionalProperties": false
            }),
            move |args| {
                let service = Arc::clone(&service);
                let group_id = group_id.clone();
                let default_thread_id = default_thread_id.clone();
                async move {
                    let action = required_str(&args, "action")?;
                    let runner_ref = required_str(&args, "runner")?;
                    let thread_id = optional_str(&args, "thread_id")
                        .unwrap_or_else(|| default_thread_id.clone());
                    let member = service
                        .store()
                        .find_member(&group_id, &runner_ref)?
                        .ok_or_else(|| anyhow!("runner not found: {runner_ref}"))?;
                    match action.as_str() {
                        "subscribe" => {
                            let mode = SubscriptionMode::parse(
                                optional_str(&args, "mode").as_deref().unwrap_or("owner"),
                            );
                            let sub =
                                service.subscribe_thread(&group_id, &thread_id, &member.id, mode)?;
                            Ok(serde_json::to_string_pretty(&json!({ "ok": true, "subscription": sub }))?)
                        }
                        "unsubscribe" => {
                            let removed =
                                service.unsubscribe_thread(&group_id, &thread_id, &member.id)?;
                            Ok(serde_json::to_string_pretty(&json!({ "ok": true, "removed": removed }))?)
                        }
                        other => bail!("unsupported subscribe action: {other}"),
                    }
                }
            },
        )
        .writes(),
    );
}

/// 注册 Chain of Recall 上下文按需召回工具（论文 3.3 节）。
///
/// 参数:
/// - `registry`: 工具注册表
/// - `service`: Super-Agent 服务
/// - `group_id`: 所属群组标识
///
/// 返回:
/// - 无
pub(super) fn register_recall_tool(
    registry: &mut ToolRegistry,
    service: Arc<SuperAgentService>,
    group_id: String,
) {
    registry.register(ToolSpec::new(
        "super_recall",
        "Read spilled full content and next-hop references for a trigger, thread, task, report, message, or member by ID (Chain of Recall).",
        json!({
            "type": "object",
            "properties": {
                "kind": { "type": "string", "enum": ["trigger", "thread", "task", "report", "message", "member"] },
                "id": { "type": "string", "description": "Target entity ID." },
                "offset": { "type": "integer", "description": "Optional char offset for paging." },
                "limit": { "type": "integer", "description": "Optional max chars to return (default 4000)." }
            },
            "required": ["kind", "id"],
            "additionalProperties": false
        }),
        move |args| {
            let service = Arc::clone(&service);
            let group_id = group_id.clone();
            async move {
                let kind = required_str(&args, "kind")?;
                let id = required_str(&args, "id")?;
                let offset = args.get("offset").and_then(Value::as_u64).unwrap_or(0) as usize;
                let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(4_000) as usize;
                let value =
                    cards::recall_object(service.store(), &group_id, &kind, &id, offset, limit)?;
                Ok(serde_json::to_string_pretty(&value)?)
            }
        },
    ));
}

/// 读取必填字符串参数。
pub(super) fn required_str(args: &Value, key: &str) -> Result<String> {
    optional_str(args, key).ok_or_else(|| anyhow!("missing required argument: {key}"))
}

/// 读取可选非空字符串参数。
pub(super) fn optional_str(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

/// 读取可选字符串数组参数。
pub(super) fn string_array(args: &Value, key: &str) -> Vec<String> {
    args.get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}
