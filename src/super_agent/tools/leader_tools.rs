use super::{
    optional_str, register_group_post_tool, register_recall_tool, register_subscribe_tool,
    required_str, string_array,
};
use crate::super_agent::cards;
use crate::super_agent::model::{MemberRole, MessageSenderKind};
use crate::super_agent::service::{SpawnRunnerRequest, SuperAgentService};
use crate::tools::{ToolRegistry, ToolSpec};
use anyhow::anyhow;
use serde_json::{json, Value};
use std::sync::Arc;

/// 【超级智能体】【Leader 工具注册】为群组常驻 Leader 注册全套编排、群聊、交付与链式召回工具。
///
/// 参数:
/// - `registry`: Leader 工具注册表
/// - `service`: Super-Agent 核心服务句柄
/// - `group_id`: 当前群组标识
/// - `leader_id`: Leader 成员标识
/// - `leader_name`: Leader 展示名称
/// - `default_thread_id`: 当前活跃线程标识
/// - `origin_channel`: 本批次来源渠道
/// - `origin_target`: 本批次来源渠道回复目标
///
/// 返回:
/// - 无
#[allow(clippy::too_many_arguments)]
pub fn register_leader_tools(
    registry: &mut ToolRegistry,
    service: Arc<SuperAgentService>,
    group_id: String,
    leader_id: String,
    leader_name: String,
    default_thread_id: String,
    origin_channel: String,
    origin_target: Value,
) {
    // 1. super_list_state：查看群组成员、任务、待审报告、Thread 订阅与绑定渠道
    {
        let service = Arc::clone(&service);
        let group_id = group_id.clone();
        registry.register(ToolSpec::new(
            "super_list_state",
            "Inspect current Super-Agent group roster (leader & runners), tasks, unsynthesized reports, threads, subscriptions, and channel bindings.",
            json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            move |_| {
                let service = Arc::clone(&service);
                let group_id = group_id.clone();
                async move {
                    let detail = service.store().group_detail(&group_id, 20)?;
                    Ok(serde_json::to_string_pretty(&json!({
                        "group": {
                            "id": detail.group.id,
                            "name": detail.group.name,
                            "workspace_path": detail.group.workspace_path,
                        },
                        "members": detail.members.iter().map(cards::member_card).collect::<Vec<_>>(),
                        "tasks": detail.tasks.iter().map(cards::task_card).collect::<Vec<_>>(),
                        "unsynthesized_reports": detail.reports.iter()
                            .filter(|rep| !rep.synthesized_by_leader)
                            .map(cards::report_card)
                            .collect::<Vec<_>>(),
                        "threads": detail.threads,
                        "subscriptions": detail.subscriptions,
                        "channels": detail.channels,
                    }))?)
                }
            },
        ));
    }

    // 2. super_spawn_runner：创建并异步启动 Worker(Runner)，立即返回不阻塞 Leader
    {
        let service = Arc::clone(&service);
        let group_id = group_id.clone();
        let default_thread_id = default_thread_id.clone();
        let origin_channel = origin_channel.clone();
        let origin_target = origin_target.clone();
        registry.register(
            ToolSpec::new(
                "super_spawn_runner",
                "Create and launch an asynchronous Worker (Runner) in this group to execute concrete work in an isolated context (and optional git worktree). Returns immediately without blocking the Leader. The runner will automatically submit a structured report back to you when finished.",
                json!({
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "description": "Short runner name, e.g. backend-dev, researcher, qa-tester." },
                        "specialty": { "type": "string", "description": "Role or specialty description." },
                        "subagent_type": { "type": "string", "description": "Optional agent profile id (e.g. general, explore)." },
                        "task_title": { "type": "string", "description": "Concise task title for the task board." },
                        "prompt": { "type": "string", "description": "Complete actionable task instructions for the runner." },
                        "thread_id": { "type": "string", "description": "Optional thread id to subscribe the runner to." }
                    },
                    "required": ["name", "prompt"],
                    "additionalProperties": false
                }),
                move |args| {
                    let service = Arc::clone(&service);
                    let group_id = group_id.clone();
                    let default_thread_id = default_thread_id.clone();
                    let origin_channel = origin_channel.clone();
                    let origin_target = origin_target.clone();
                    async move {
                        let name = required_str(&args, "name")?;
                        let prompt = required_str(&args, "prompt")?;
                        let specialty = optional_str(&args, "specialty").unwrap_or_default();
                        let subagent_type = optional_str(&args, "subagent_type")
                            .unwrap_or_else(|| "general".to_string());
                        let task_title = optional_str(&args, "task_title")
                            .unwrap_or_else(|| format!("Task for {name}"));
                        let thread_id = optional_str(&args, "thread_id")
                            .unwrap_or_else(|| default_thread_id.clone());
                        let (member, task) = service
                            .spawn_runner(SpawnRunnerRequest {
                                group_id,
                                name,
                                specialty,
                                subagent_type,
                                task_title,
                                prompt,
                                thread_id: Some(thread_id),
                                origin_channel: Some(origin_channel),
                                origin_target: Some(origin_target),
                            })
                            .await?;
                        Ok(serde_json::to_string_pretty(&json!({
                            "ok": true,
                            "runner": cards::member_card(&member),
                            "task": cards::task_card(&task),
                            "note": "Runner started asynchronously. Continue responding to the user; you will be automatically woken when the runner submits its report."
                        }))?)
                    }
                },
            )
            .writes(),
        );
    }

    // 3. super_dispatch_task：向已有 Runner 派发新任务（或按名自动新建）
    {
        let service = Arc::clone(&service);
        let group_id = group_id.clone();
        let default_thread_id = default_thread_id.clone();
        let origin_channel = origin_channel.clone();
        let origin_target = origin_target.clone();
        registry.register(
            ToolSpec::new(
                "super_dispatch_task",
                "Create a tracked task and dispatch it to an existing runner (by runner_id or name), or spawn a new runner if none matches.",
                json!({
                    "type": "object",
                    "properties": {
                        "runner": { "type": "string", "description": "Existing runner id or name, or new runner name." },
                        "task_title": { "type": "string", "description": "Task title." },
                        "objective": { "type": "string", "description": "Detailed task instructions." },
                        "thread_id": { "type": "string", "description": "Optional thread id." }
                    },
                    "required": ["runner", "task_title", "objective"],
                    "additionalProperties": false
                }),
                move |args| {
                    let service = Arc::clone(&service);
                    let group_id = group_id.clone();
                    let default_thread_id = default_thread_id.clone();
                    let origin_channel = origin_channel.clone();
                    let origin_target = origin_target.clone();
                    async move {
                        let runner_ref = required_str(&args, "runner")?;
                        let title = required_str(&args, "task_title")?;
                        let objective = required_str(&args, "objective")?;
                        let thread_id = optional_str(&args, "thread_id")
                            .unwrap_or_else(|| default_thread_id.clone());
                        let existing = service.store().find_member(&group_id, &runner_ref)?;
                        if let Some(member) = existing {
                            if member.status.is_live() && member.role == MemberRole::Worker {
                                let task = service
                                    .dispatch_task_to_runner(
                                        &group_id,
                                        &member.id,
                                        &title,
                                        &objective,
                                        &thread_id,
                                        &origin_channel,
                                        origin_target,
                                    )
                                    .await?;
                                return Ok(serde_json::to_string_pretty(&json!({
                                    "ok": true,
                                    "runner": cards::member_card(&member),
                                    "task": cards::task_card(&task),
                                }))?);
                            }
                        }
                        let (member, task) = service
                            .spawn_runner(SpawnRunnerRequest {
                                group_id,
                                name: runner_ref,
                                specialty: String::new(),
                                subagent_type: "general".to_string(),
                                task_title: title,
                                prompt: objective,
                                thread_id: Some(thread_id),
                                origin_channel: Some(origin_channel),
                                origin_target: Some(origin_target),
                            })
                            .await?;
                        Ok(serde_json::to_string_pretty(&json!({
                            "ok": true,
                            "runner": cards::member_card(&member),
                            "task": cards::task_card(&task),
                        }))?)
                    }
                },
            )
            .writes(),
        );
    }

    // 4. super_message_runner：向指定 Runner 发送追问、修正或上下文补充
    {
        let service = Arc::clone(&service);
        let group_id = group_id.clone();
        let leader_name = leader_name.clone();
        registry.register(
            ToolSpec::new(
                "super_message_runner",
                "Send a follow-up instruction or clarification directly to a Runner in this group.",
                json!({
                    "type": "object",
                    "properties": {
                        "runner": { "type": "string", "description": "Runner member_id or name." },
                        "message": { "type": "string", "description": "Message to inject into the runner." }
                    },
                    "required": ["runner", "message"],
                    "additionalProperties": false
                }),
                move |args| {
                    let service = Arc::clone(&service);
                    let group_id = group_id.clone();
                    let leader_name = leader_name.clone();
                    async move {
                        let runner_ref = required_str(&args, "runner")?;
                        let message = required_str(&args, "message")?;
                        let member = service
                            .store()
                            .find_member(&group_id, &runner_ref)?
                            .ok_or_else(|| anyhow!("runner not found: {runner_ref}"))?;
                        service
                            .message_runner(&group_id, &member.id, &leader_name, &message)
                            .await?;
                        Ok(serde_json::to_string_pretty(&json!({
                            "ok": true,
                            "runner_id": member.id,
                            "runner_name": member.name,
                        }))?)
                    }
                },
            )
            .writes(),
        );
    }

    // 5. super_stop_runner：终止指定 Runner
    {
        let service = Arc::clone(&service);
        let group_id = group_id.clone();
        registry.register(
            ToolSpec::new(
                "super_stop_runner",
                "Stop a running or idle Runner in this group.",
                json!({
                    "type": "object",
                    "properties": {
                        "runner": { "type": "string", "description": "Runner member_id or name." },
                        "reason": { "type": "string", "description": "Optional reason for stopping." }
                    },
                    "required": ["runner"],
                    "additionalProperties": false
                }),
                move |args| {
                    let service = Arc::clone(&service);
                    let group_id = group_id.clone();
                    async move {
                        let runner_ref = required_str(&args, "runner")?;
                        let reason = optional_str(&args, "reason")
                            .unwrap_or_else(|| "stopped by leader".to_string());
                        let member = service
                            .store()
                            .find_member(&group_id, &runner_ref)?
                            .ok_or_else(|| anyhow!("runner not found: {runner_ref}"))?;
                        let updated = service.stop_runner(&group_id, &member.id, &reason)?;
                        Ok(serde_json::to_string_pretty(&json!({
                            "ok": true,
                            "runner": cards::member_card(&updated),
                        }))?)
                    }
                },
            )
            .writes(),
        );
    }

    // 6. super_group_post：群内发言或 @成员协同
    register_group_post_tool(
        registry,
        Arc::clone(&service),
        group_id.clone(),
        default_thread_id.clone(),
        MessageSenderKind::Leader,
        leader_id,
        leader_name,
    );

    // 7. super_deliver_result：归纳 Runner 工作成果，统一向用户与来源渠道正式递交
    {
        let service = Arc::clone(&service);
        let group_id = group_id.clone();
        let default_thread_id = default_thread_id.clone();
        let origin_channel = origin_channel.clone();
        let origin_target = origin_target.clone();
        registry.register(
            ToolSpec::new(
                "super_deliver_result",
                "Synthesize completed Runner work and proactively deliver the final outcome to the user in the group and back to the originating external channel (Weixin/Feishu/QQ/OneBot).",
                json!({
                    "type": "object",
                    "properties": {
                        "task_id": { "type": "string", "description": "Optional primary task_id being delivered." },
                        "report_ids": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Optional report_ids synthesized into this deliverable."
                        },
                        "summary": { "type": "string", "description": "Concise executive summary (used for IM channel push)." },
                        "deliverable": { "type": "string", "description": "Complete synthesized deliverable posted to the group." },
                        "channel": { "type": "string", "description": "Optional explicit target channel override." }
                    },
                    "required": ["summary", "deliverable"],
                    "additionalProperties": false
                }),
                move |args| {
                    let service = Arc::clone(&service);
                    let group_id = group_id.clone();
                    let default_thread_id = default_thread_id.clone();
                    let origin_channel = origin_channel.clone();
                    let origin_target = origin_target.clone();
                    async move {
                        let task_id = optional_str(&args, "task_id");
                        let report_ids = string_array(&args, "report_ids");
                        let summary = required_str(&args, "summary")?;
                        let deliverable = required_str(&args, "deliverable")?;
                        let channel_override = optional_str(&args, "channel");
                        let (msg, receipt) = service
                            .deliver_leader_result(
                                &group_id,
                                &default_thread_id,
                                task_id.as_deref(),
                                &report_ids,
                                &summary,
                                &deliverable,
                                channel_override.as_deref(),
                                Some(origin_channel.as_str()),
                                Some(&origin_target),
                            )
                            .await?;
                        Ok(serde_json::to_string_pretty(&json!({
                            "ok": true,
                            "message_id": msg.id,
                            "delivery_receipt": receipt,
                        }))?)
                    }
                },
            )
            .writes(),
        );
    }

    // 8. super_subscribe_thread & super_recall
    register_subscribe_tool(
        registry,
        Arc::clone(&service),
        group_id.clone(),
        default_thread_id,
    );
    register_recall_tool(registry, service, group_id);
}
