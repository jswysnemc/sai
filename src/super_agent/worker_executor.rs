use super::model::{MemberStatus, ReportStatus};
use super::prompts::worker_system_prompt;
use super::service::SuperAgentService;
use super::tools::{register_worker_tools, role_runtime_config};
use crate::agent::AgentMode;
use crate::config::AppConfig;
use crate::llm::OpenAiCompatibleClient;
use crate::tools::{ProgressMode, SubagentInjectedMessage, SubagentProgress, SubagentRunner, ToolProgress};
use anyhow::Result;
use serde_json::json;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::oneshot;

const WORKER_EXECUTION_TIMEOUT_SECS: u64 = 1800;
const WORKER_TOOL_TIMEOUT_SECS: u64 = 120;

/// 进程内单个活跃 Worker(Runner) 的控制句柄。
struct LiveWorkerHandle {
    inbox: Arc<Mutex<VecDeque<SubagentInjectedMessage>>>,
    cancel_tx: Option<oneshot::Sender<()>>,
}

/// 【超级智能体】【Worker 执行器】管理上下文隔离的并发 Runner、步间消息注入与隔离 Worktree。
#[derive(Default)]
pub struct WorkerExecutor {
    handles: Mutex<HashMap<String, LiveWorkerHandle>>,
}

impl WorkerExecutor {
    /// 创建 Worker 执行管理器。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 空执行管理器
    pub fn new() -> Self {
        Self::default()
    }

    /// 判断指定 Worker 当前是否正在本进程运行。
    ///
    /// 参数:
    /// - `member_id`: Worker 成员标识
    ///
    /// 返回:
    /// - 正在本进程运行时返回 true
    pub fn is_live(&self, member_id: &str) -> bool {
        self.handles
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .contains_key(member_id)
    }

    /// 向正在运行的 Worker 步间信箱投递一条追问或订阅直通消息。
    ///
    /// 参数:
    /// - `member_id`: 目标 Worker 标识
    /// - `from`: 发送人标识
    /// - `text`: 消息正文
    ///
    /// 返回:
    /// - 成功投递到活跃信箱返回 true；Worker 当前未在运行返回 false
    pub fn enqueue_live_message(&self, member_id: &str, from: &str, text: &str) -> bool {
        let guard = self.handles.lock().unwrap_or_else(|err| err.into_inner());
        let Some(handle) = guard.get(member_id) else {
            return false;
        };
        let mut inbox = handle.inbox.lock().unwrap_or_else(|err| err.into_inner());
        inbox.push_back(SubagentInjectedMessage {
            from: from.to_string(),
            text: text.to_string(),
        });
        true
    }

    /// 触发取消信号停止正在运行的 Worker。
    ///
    /// 参数:
    /// - `member_id`: 目标 Worker 标识
    ///
    /// 返回:
    /// - 是否命中了正在运行的句柄
    pub fn cancel_live(&self, member_id: &str) -> bool {
        let mut guard = self.handles.lock().unwrap_or_else(|err| err.into_inner());
        if let Some(mut handle) = guard.remove(member_id) {
            if let Some(tx) = handle.cancel_tx.take() {
                let _ = tx.send(());
            }
            return true;
        }
        false
    }

    /// 异步启动 Worker(Runner) 执行指定任务，完成后自动确保提交结构化报告给 Leader。
    ///
    /// 参数:
    /// - `service`: Super-Agent 核心服务句柄
    /// - `group_id`: 所属群组标识
    /// - `member_id`: Worker 成员标识
    /// - `thread_id`: 归属线程标识
    /// - `task_id`: 关联任务标识
    /// - `prompt`: 完整任务指令
    ///
    /// 返回:
    /// - 无
    pub fn launch(
        &self,
        service: Arc<SuperAgentService>,
        group_id: String,
        member_id: String,
        thread_id: String,
        task_id: Option<String>,
        prompt: String,
    ) {
        let inbox = Arc::new(Mutex::new(VecDeque::<SubagentInjectedMessage>::new()));
        let (cancel_tx, cancel_rx) = oneshot::channel::<()>();
        {
            let mut guard = self.handles.lock().unwrap_or_else(|err| err.into_inner());
            if let Some(mut previous) = guard.remove(&member_id) {
                if let Some(tx) = previous.cancel_tx.take() {
                    let _ = tx.send(());
                }
            }
            guard.insert(
                member_id.clone(),
                LiveWorkerHandle {
                    inbox: Arc::clone(&inbox),
                    cancel_tx: Some(cancel_tx),
                },
            );
        }
        tokio::spawn(async move {
            run_worker_task(
                service,
                group_id,
                member_id,
                thread_id,
                task_id,
                prompt,
                inbox,
                cancel_rx,
            )
            .await;
        });
    }

    /// 清理已退出的 Worker 句柄。
    ///
    /// 参数:
    /// - `member_id`: Worker 成员标识
    ///
    /// 返回:
    /// - 无
    fn remove_handle(&self, member_id: &str) {
        let mut guard = self.handles.lock().unwrap_or_else(|err| err.into_inner());
        guard.remove(member_id);
    }
}

/// 在异步任务中执行单个 Worker(Runner) 会话。
#[allow(clippy::too_many_arguments)]
async fn run_worker_task(
    service: Arc<SuperAgentService>,
    group_id: String,
    member_id: String,
    thread_id: String,
    task_id: Option<String>,
    prompt: String,
    inbox: Arc<Mutex<VecDeque<SubagentInjectedMessage>>>,
    mut cancel_rx: oneshot::Receiver<()>,
) {
    // 1. 读取群组、成员与角色配置，准备独立执行目录或隔离 worktree
    let setup = prepare_worker_execution(&service, &group_id, &member_id, &thread_id, task_id.as_deref());
    let (worker_name, runtime_cwd, worktree, runner, reported_flag) = match setup {
        Ok(ready) => ready,
        Err(error) => {
            let err_text = format!("{error:#}");
            let worker_name = service
                .store()
                .get_member(&member_id)
                .ok()
                .flatten()
                .map(|m| m.name)
                .unwrap_or_else(|| member_id.clone());
            let _ = service.store().update_member_state(
                &member_id,
                MemberStatus::Failed,
                Some(&err_text),
                Some(None),
            );
            let _ = service
                .submit_runner_report(
                    &group_id,
                    &thread_id,
                    task_id.as_deref(),
                    &member_id,
                    &worker_name,
                    ReportStatus::Failed,
                    &format!("Runner failed to start: {err_text}"),
                    &err_text,
                    Vec::new(),
                    None,
                )
                .await;
            service.worker_executor().remove_handle(&member_id);
            return;
        }
    };

    // 2. 注入步间信箱轮询，使 Fast-Path 订阅直通消息可在下一推理步即时送达
    let inbox_poll = Arc::clone(&inbox);
    let runner = runner.with_message_poll(Arc::new(move || {
        let mut guard = inbox_poll.lock().unwrap_or_else(|err| err.into_inner());
        guard.drain(..).collect()
    }));

    // 3. 启动轻量心跳更新，供 Watchdog 识别运行存活
    let heartbeat_service = Arc::clone(&service);
    let heartbeat_member_id = member_id.clone();
    let heartbeat_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(15));
        loop {
            interval.tick().await;
            let _ = heartbeat_service.store().update_member_state(
                &heartbeat_member_id,
                MemberStatus::Running,
                None,
                None,
            );
        }
    });

    let _ = service.store().update_member_state(
        &member_id,
        MemberStatus::Running,
        Some("executing task"),
        Some(task_id.as_deref()),
    );
    service.emit_event(
        "runner.started",
        Some(&group_id),
        json!({ "member_id": member_id, "name": worker_name, "task_id": task_id }),
    );

    // 4. 在独立工作目录作用域内运行完整 ReAct Loop，支持随时取消与总时限保护
    let run_result = crate::runtime_cwd::scope(runtime_cwd, async {
        tokio::select! {
            _ = &mut cancel_rx => Err(anyhow::anyhow!("cancelled")),
            outcome = tokio::time::timeout(
                Duration::from_secs(WORKER_EXECUTION_TIMEOUT_SECS),
                runner.run(&prompt),
            ) => match outcome {
                Ok(inner) => inner,
                Err(_) => Err(anyhow::anyhow!("runner timed out after {WORKER_EXECUTION_TIMEOUT_SECS}s")),
            }
        }
    })
    .await;

    heartbeat_task.abort();
    service.worker_executor().remove_handle(&member_id);

    // 5. 若使用了隔离 worktree，成功时自动合并改动并清理临时分支
    let merge_note = finalize_worker_worktree(worktree.as_ref(), run_result.is_ok());
    let already_reported = reported_flag.load(Ordering::Acquire);

    match run_result {
        Ok((chat_result, stats)) => {
            let final_text = if merge_note.is_empty() {
                chat_result.content
            } else {
                format!("{}\n\n{merge_note}", chat_result.content)
            };
            let _ = service.store().update_member_state(
                &member_id,
                MemberStatus::Idle,
                Some(&format!(
                    "completed ({} tool calls, {} tokens)",
                    stats.tool_calls, stats.token_estimate
                )),
                Some(None),
            );
            // 6. 兜底自动提交结构化报告：即使 Runner 未显式调用 super_report_submit，也确保成果上报给 Leader
            if !already_reported {
                let summary = summarize_output(&final_text);
                let _ = service
                    .submit_runner_report(
                        &group_id,
                        &thread_id,
                        task_id.as_deref(),
                        &member_id,
                        &worker_name,
                        ReportStatus::Ready,
                        &summary,
                        &final_text,
                        Vec::new(),
                        None,
                    )
                    .await;
            }
            service.emit_event(
                "runner.completed",
                Some(&group_id),
                json!({ "member_id": member_id, "name": worker_name, "task_id": task_id }),
            );
        }
        Err(error) if error.to_string() == "cancelled" => {
            let _ = service.store().update_member_state(
                &member_id,
                MemberStatus::Stopped,
                Some("stopped"),
                Some(None),
            );
            service.emit_event(
                "runner.stopped",
                Some(&group_id),
                json!({ "member_id": member_id, "name": worker_name, "task_id": task_id }),
            );
        }
        Err(error) => {
            let err_text = format!("{error:#}");
            let _ = service.store().update_member_state(
                &member_id,
                MemberStatus::Failed,
                Some(&err_text),
                Some(None),
            );
            if !already_reported {
                let _ = service
                    .submit_runner_report(
                        &group_id,
                        &thread_id,
                        task_id.as_deref(),
                        &member_id,
                        &worker_name,
                        ReportStatus::Failed,
                        &format!("Runner {worker_name} failed: {}", summarize_output(&err_text)),
                        &err_text,
                        Vec::new(),
                        None,
                    )
                    .await;
            }
            service.emit_event(
                "runner.failed",
                Some(&group_id),
                json!({ "member_id": member_id, "name": worker_name, "error": err_text }),
            );
        }
    }
}

/// 组装 Worker 运行客户端、隔离工作目录与工具集。
#[allow(clippy::type_complexity)]
fn prepare_worker_execution(
    service: &Arc<SuperAgentService>,
    group_id: &str,
    member_id: &str,
    thread_id: &str,
    task_id: Option<&str>,
) -> Result<(
    String,
    PathBuf,
    Option<crate::tools::subagent_worktree::SubagentWorktree>,
    SubagentRunner,
    Arc<AtomicBool>,
)> {
    let paths = service.paths();
    let group = service
        .store()
        .get_group(group_id)?
        .ok_or_else(|| anyhow::anyhow!("group not found: {group_id}"))?;
    let worker = service
        .store()
        .get_member(member_id)?
        .ok_or_else(|| anyhow::anyhow!("worker not found: {member_id}"))?;
    let leader_name = service
        .store()
        .get_member(&group.leader_member_id)?
        .map(|member| member.name)
        .unwrap_or_else(|| "Leader".to_string());
    let base_config = AppConfig::load_or_default(paths)?;
    let worker_config = role_runtime_config(&base_config, false);
    let client = OpenAiCompatibleClient::from_config(&worker_config, paths)?;

    // 1. 决定工作目录并在开启 worktree 时创建隔离分支
    let base_workdir = if !worker.workdir.trim().is_empty() {
        PathBuf::from(&worker.workdir)
    } else if !group.workspace_path.trim().is_empty() {
        PathBuf::from(&group.workspace_path)
    } else {
        crate::runtime_cwd::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    };
    let worktree = if base_config.super_agent.worker_use_worktree {
        crate::tools::subagent_worktree::try_create(&base_workdir, &worker.name)
            .ok()
            .flatten()
    } else {
        None
    };
    let runtime_cwd = if let Some(ref wt) = worktree {
        let _ = service.store().update_member_workdir(
            member_id,
            &wt.workdir.display().to_string(),
            Some(&wt.branch_name),
        );
        wt.workdir.clone()
    } else {
        base_workdir
    };

    // 2. 构建 Worker 完整执行工具表（文件、搜索、命令 + Super-Agent 汇报与群聊工具）
    let mut registry = crate::cli::build_tool_registry_without_mcp(&worker_config, paths, AgentMode::Yolo)?;
    let reported_flag = Arc::new(AtomicBool::new(false));
    register_worker_tools(
        &mut registry,
        Arc::clone(service),
        group_id.to_string(),
        member_id.to_string(),
        worker.name.clone(),
        thread_id.to_string(),
        task_id.map(ToOwned::to_owned),
        Arc::clone(&reported_flag),
    );
    let system_prompt = worker_system_prompt(&group, &worker, &leader_name, task_id, thread_id);
    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let progress_service = Arc::clone(service);
    let progress_member_id = member_id.to_string();
    let progress_group_id = group_id.to_string();
    tokio::spawn(async move {
        while let Some(line) = progress_rx.recv().await {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('{') {
                let preview = summarize_output(trimmed);
                let _ = progress_service.store().update_member_state(
                    &progress_member_id,
                    MemberStatus::Running,
                    Some(&preview),
                    None,
                );
                progress_service.emit_event(
                    "runner.progress",
                    Some(&progress_group_id),
                    json!({ "member_id": progress_member_id, "activity": preview }),
                );
            }
        }
    });
    let progress = SubagentProgress::new(
        ToolProgress::new(progress_tx),
        ProgressMode::Summary,
        true,
    );
    let runner = SubagentRunner::new(client, system_prompt, registry, progress)
        .with_hook_agent_type("super_worker")
        .timeout_seconds(WORKER_TOOL_TIMEOUT_SECS)
        .session_deadline_seconds(WORKER_EXECUTION_TIMEOUT_SECS);
    Ok((worker.name, runtime_cwd, worktree, runner, reported_flag))
}

/// 合并并清理隔离 worktree。
fn finalize_worker_worktree(
    worktree: Option<&crate::tools::subagent_worktree::SubagentWorktree>,
    succeeded: bool,
) -> String {
    let Some(wt) = worktree else {
        return String::new();
    };
    let mut note = String::new();
    if succeeded {
        match crate::tools::subagent_worktree::apply(wt) {
            Ok(apply) if apply.changed && apply.applied => {
                note = format!(
                    "[worktree branch {} merged via {}]",
                    wt.branch_name,
                    apply.apply_method.as_deref().unwrap_or("patch")
                );
            }
            Ok(apply) if apply.changed => {
                note = format!("[worktree branch {} had changes; auto-merge skipped]", wt.branch_name);
            }
            Err(err) => {
                note = format!("[worktree merge failed: {err}]");
            }
            _ => {}
        }
    }
    let _ = crate::tools::subagent_worktree::cleanup(wt);
    note
}

/// 从输出正文提取首段摘要。
fn summarize_output(text: &str) -> String {
    let first = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("Completed");
    let mut chars = first.chars();
    let head: String = chars.by_ref().take(180).collect();
    if chars.next().is_some() {
        format!("{head}...")
    } else {
        head
    }
}
