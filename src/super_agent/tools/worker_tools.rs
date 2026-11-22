use super::{
    optional_str, register_group_post_tool, register_recall_tool, register_subscribe_tool,
    required_str, string_array,
};
use crate::super_agent::cards;
use crate::super_agent::model::{MessageSenderKind, ReportStatus};
use crate::super_agent::service::SuperAgentService;
use crate::tools::{ToolRegistry, ToolSpec};
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// 【超级智能体】【Worker 工具注册】为独立执行的 Worker(Runner) 注册结构化汇报、群组沟通、订阅与召回工具。
///
/// 参数:
/// - `registry`: Worker 工具注册表
/// - `service`: Super-Agent 核心服务句柄
/// - `group_id`: 所属群组标识
/// - `worker_id`: 当前 Runner 成员标识
/// - `worker_name`: 当前 Runner 展示名
/// - `thread_id`: 所属 Thread 标识
/// - `task_id`: 关联任务标识
/// - `reported_flag`: 标记本轮是否已显式调用 `super_report_submit`
///
/// 返回:
/// - 无
#[allow(clippy::too_many_arguments)]
pub fn register_worker_tools(
    registry: &mut ToolRegistry,
    service: Arc<SuperAgentService>,
    group_id: String,
    worker_id: String,
    worker_name: String,
    thread_id: String,
    task_id: Option<String>,
    reported_flag: Arc<AtomicBool>,
) {
    // 1. super_report_submit：向 Leader 提交结构化成果/进度/阻塞报告
    {
        let service = Arc::clone(&service);
        let group_id = group_id.clone();
        let worker_id = worker_id.clone();
        let worker_name = worker_name.clone();
        let thread_id = thread_id.clone();
        let task_id = task_id.clone();
        let reported_flag = Arc::clone(&reported_flag);
        registry.register(
            ToolSpec::new(
                "super_report_submit",
                "Submit a structured work report back to the group Leader. Call this when your task is ready, blocked, failed, waiting for a human decision, or to share a milestone progress update.",
                json!({
                    "type": "object",
                    "properties": {
                        "status": {
                            "type": "string",
                            "enum": ["progress", "waiting", "ready", "blocked", "failed", "dropped"],
                            "description": "Structured report state."
                        },
                        "summary": { "type": "string", "description": "Concise one-paragraph summary for the Leader." },
                        "details": { "type": "string", "description": "Detailed findings, code changes, verification commands and outputs." },
                        "artifacts": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Optional list of modified files, paths, or output identifiers."
                        },
                        "needs_human_question": {
                            "type": "string",
                            "description": "Optional concrete question when status is waiting or blocked."
                        }
                    },
                    "required": ["status", "summary"],
                    "additionalProperties": false
                }),
                move |args| {
                    let service = Arc::clone(&service);
                    let group_id = group_id.clone();
                    let worker_id = worker_id.clone();
                    let worker_name = worker_name.clone();
                    let thread_id = thread_id.clone();
                    let task_id = task_id.clone();
                    let reported_flag = Arc::clone(&reported_flag);
                    async move {
                        let status = ReportStatus::parse(&required_str(&args, "status")?);
                        let summary = required_str(&args, "summary")?;
                        let details = optional_str(&args, "details")
                            .unwrap_or_else(|| summary.clone());
                        let artifacts = string_array(&args, "artifacts");
                        let needs_human_question = optional_str(&args, "needs_human_question");
                        let report = service
                            .submit_runner_report(
                                &group_id,
                                &thread_id,
                                task_id.as_deref(),
                                &worker_id,
                                &worker_name,
                                status,
                                &summary,
                                &details,
                                artifacts,
                                needs_human_question,
                            )
                            .await?;
                        if status.requires_leader_synthesis() {
                            reported_flag.store(true, Ordering::Release);
                        }
                        Ok(serde_json::to_string_pretty(&json!({
                            "ok": true,
                            "report": cards::report_card(&report),
                        }))?)
                    }
                },
            )
            .writes(),
        );
    }

    // 2. super_group_post：Runner 在群组内与 Leader 或其他成员交流
    register_group_post_tool(
        registry,
        Arc::clone(&service),
        group_id.clone(),
        thread_id.clone(),
        MessageSenderKind::Worker,
        worker_id,
        worker_name,
    );

    // 3. super_subscribe_thread & super_recall
    register_subscribe_tool(registry, Arc::clone(&service), group_id.clone(), thread_id);
    register_recall_tool(registry, service, group_id);
}
