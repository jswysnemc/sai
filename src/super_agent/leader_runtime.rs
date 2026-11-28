use super::cards;
use super::model::{
    Group, MemberStatus, MessageKind, MessageSenderKind, RunnerReport, TriggerDisposition,
    TriggerEvent,
};
use super::prompts::leader_system_prompt;
use super::service::SuperAgentService;
use super::tools::{is_leader_excluded_tool, register_leader_tools, role_runtime_config};
use crate::agent::{AgentEvent, AgentMode};
use crate::config::AppConfig;
use crate::llm::ChatStreamKind;
use crate::paths::SaiPaths;
use crate::runner::{
    RunnerEvent, RunnerSubmission, SessionRunner, SubmissionSource, UserInputSubmission,
};
use crate::tools::ToolRegistry;
use anyhow::Result;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// 【超级智能体】【Leader 会话】确保群组常驻 Leader 对应的稳定会话已在独立作用域中建档。
///
/// 会话落在 `state_dir/super-agent/leader-sessions` 独立作用域，既能被 `StateStore::for_session`
/// 跨工作区稳定检索，又不会污染用户日常项目工作区的 `/sessions` 列表。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `group`: 目标群组
///
/// 返回:
/// - 已建档的稳定会话 ID
pub fn ensure_leader_session(paths: &SaiPaths, group: &Group) -> Result<String> {
    let scope_dir = paths.state_dir.join("super-agent").join("leader-sessions");
    std::fs::create_dir_all(&scope_dir)?;
    let session_id = format!("super-leader-{}", group.id);
    let title = format!("Super-Agent Leader · {}", group.name);
    let session = crate::state::ensure_workspace_session(paths, &scope_dir, &session_id, &title)?;
    Ok(session.id)
}

/// 【超级智能体】【Leader 工具构建】构造 Leader 专用工具表：排除重执行/冲突工具并注入 9 个编排工具。
///
/// 参数:
/// - `leader_config`: Leader 运行期配置
/// - `paths`: Sai 路径集合
/// - `service`: Super-Agent 核心服务句柄
/// - `group_id`: 当前群组 ID
/// - `leader_id`: Leader 成员 ID
/// - `leader_name`: Leader 展示名称
/// - `active_thread_id`: 当前主线程 ID
/// - `origin_channel`: 当前批次来源渠道
/// - `origin_target`: 当前批次来源回复目标
///
/// 返回:
/// - 已过滤并注入编排工具的注册表
#[allow(clippy::too_many_arguments)]
pub fn build_leader_tool_registry(
    leader_config: &AppConfig,
    paths: &SaiPaths,
    service: Arc<SuperAgentService>,
    group_id: String,
    leader_id: String,
    leader_name: String,
    active_thread_id: String,
    origin_channel: String,
    origin_target: Value,
) -> Result<ToolRegistry> {
    let mut registry =
        crate::cli::build_tool_registry_without_mcp(leader_config, paths, AgentMode::Yolo)?
            .clone_excluding_predicate(is_leader_excluded_tool);
    register_leader_tools(
        &mut registry,
        service,
        group_id,
        leader_id,
        leader_name,
        active_thread_id,
        origin_channel,
        origin_target,
    );
    Ok(registry)
}

/// 【超级智能体】【Leader 批处理调度器】保证每个群组同一时刻最多运行一个常驻 Leader 回合，
/// 运行期间到达的新 Trigger 与 Runner 报告自动积压，在下一轮一并批处理（论文 2.1 & 7.2 节）。
#[derive(Default)]
pub struct LeaderCoordinator {
    active_groups: Mutex<HashSet<String>>,
    pending_recheck: Mutex<HashSet<String>>,
}

impl LeaderCoordinator {
    /// 创建 Leader 批处理协调器。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 新建协调器
    pub fn new() -> Self {
        Self::default()
    }

    /// 唤醒指定群组的 Leader 处理积压的 Trigger 或 Runner 报告。
    ///
    /// 若该群组 Leader 正在推理，只置位 `pending_recheck`，待本轮结束后自动合并积压批次继续执行。
    ///
    /// 参数:
    /// - `service`: Super-Agent 核心服务句柄
    /// - `group_id`: 目标群组标识
    ///
    /// 返回:
    /// - 无
    pub fn wake(&self, service: Arc<SuperAgentService>, group_id: String) {
        let should_spawn = {
            let mut active = self
                .active_groups
                .lock()
                .unwrap_or_else(|err| err.into_inner());
            if active.contains(&group_id) {
                self.pending_recheck
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .insert(group_id.clone());
                false
            } else {
                active.insert(group_id.clone());
                true
            }
        };
        if !should_spawn {
            return;
        }
        tokio::spawn(async move {
            run_leader_drain_loop(service, group_id).await;
        });
    }

    /// 判断群组 Leader 当前是否正在运行。
    ///
    /// 参数:
    /// - `group_id`: 群组标识
    ///
    /// 返回:
    /// - 正在执行推理回合返回 true
    pub fn is_busy(&self, group_id: &str) -> bool {
        self.active_groups
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .contains(group_id)
    }

    /// 在单轮结束后决定是否继续下一轮批处理或释放群组活跃锁。
    ///
    /// 参数:
    /// - `group_id`: 群组标识
    ///
    /// 返回:
    /// - true 表示仍有待处理积压需要继续循环
    fn finish_or_continue(&self, group_id: &str) -> bool {
        let mut pending = self
            .pending_recheck
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        if pending.remove(group_id) {
            return true;
        }
        self.active_groups
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .remove(group_id);
        false
    }

    /// 发生错误时直接释放群组活跃锁与重查标记，等待后续显式事件再次唤醒。
    fn release(&self, group_id: &str) {
        self.pending_recheck
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .remove(group_id);
        self.active_groups
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .remove(group_id);
    }
}

/// 排空群组 Leader 的全部积压 Trigger 与待归纳 Runner 报告。
async fn run_leader_drain_loop(service: Arc<SuperAgentService>, group_id: String) {
    loop {
        if let Err(error) = run_single_leader_batch(&service, &group_id).await {
            let err_text = format!("{error:#}");
            if let Ok(Some(group)) = service.store().get_group(&group_id) {
                let _ = service.store().update_member_state(
                    &group.leader_member_id,
                    MemberStatus::Idle,
                    Some(&format!("leader turn failed: {err_text}")),
                    None,
                );
                let _ = service.record_group_message(super::store::NewGroupMessage {
                    group_id: group_id.clone(),
                    thread_id: group.default_thread_id,
                    sender_kind: MessageSenderKind::System,
                    sender_id: "system".to_string(),
                    sender_name: "System".to_string(),
                    recipient_id: None,
                    recipient_name: None,
                    kind: MessageKind::Alert,
                    channel_source: None,
                    content: format!("Leader 执行失败：{err_text}"),
                    summary: None,
                    related_task_id: None,
                    related_report_id: None,
                });
            }
            // 出错后主动退避退出循环，避免同一批未消费报告触发无限重试循环
            service.leader_coordinator().release(&group_id);
            break;
        }
        // 1. 检查是否还有新一轮积压 Trigger 或未归纳报告
        let has_more = service
            .store()
            .list_queued_leader_triggers(&group_id, 1)
            .map(|items| !items.is_empty())
            .unwrap_or(false)
            || service
                .store()
                .list_unsynthesized_reports(Some(&group_id))
                .map(|items| !items.is_empty())
                .unwrap_or(false);
        if has_more {
            continue;
        }
        if !service.leader_coordinator().finish_or_continue(&group_id) {
            break;
        }
    }
}

/// 执行单次 Leader 批处理推理回合。
///
/// 参数:
/// - `service`: Super-Agent 服务
/// - `group_id`: 群组标识
///
/// 返回:
/// - 本轮处理是否成功
pub async fn run_single_leader_batch(service: &Arc<SuperAgentService>, group_id: &str) -> Result<()> {
    let paths = service.paths();
    let base_config = AppConfig::load_or_default(paths)?;
    let batch_limit = base_config.super_agent.leader_batch_limit.max(1);
    let triggers = service
        .store()
        .list_queued_leader_triggers(group_id, batch_limit)?;
    let unsynthesized_reports = service.store().list_unsynthesized_reports(Some(group_id))?;
    if triggers.is_empty() && unsynthesized_reports.is_empty() {
        return Ok(());
    }

    let group = service
        .store()
        .get_group(group_id)?
        .ok_or_else(|| anyhow::anyhow!("group not found: {group_id}"))?;
    let members = service.store().list_members(group_id)?;
    let leader = members
        .iter()
        .find(|member| member.id == group.leader_member_id)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("leader member missing for group {group_id}"))?;

    // 1. 标记本批 Trigger 进入 batched 状态，避免重复消费
    let trigger_ids: Vec<String> = triggers.iter().map(|item| item.id.clone()).collect();
    service.store().update_trigger_disposition(
        &trigger_ids,
        TriggerDisposition::Batched,
        Some(&format!("batched({})", trigger_ids.len())),
    )?;

    let _ = service.store().update_member_state(
        &leader.id,
        MemberStatus::Running,
        Some(&format!(
            "orchestrating {} trigger(s), {} report(s)",
            triggers.len(),
            unsynthesized_reports.len()
        )),
        None,
    );
    service.emit_event(
        "leader.thinking",
        Some(group_id),
        json!({
            "member_id": leader.id,
            "batch_triggers": triggers.len(),
            "pending_reports": unsynthesized_reports.len(),
        }),
    );

    // 2. 确定本批次主线程与默认来源渠道（供 Leader 创建的任务与交付回推继承）
    let active_thread_id = triggers
        .last()
        .map(|item| item.thread_id.clone())
        .or_else(|| unsynthesized_reports.last().map(|rep| rep.thread_id.clone()))
        .unwrap_or_else(|| group.default_thread_id.clone());
    let (origin_channel, origin_target) = resolve_batch_origin(&triggers, &unsynthesized_reports, service);

    // 3. 记录本轮开始前的消息 seq，用于判断 Leader 是否已通过工具主动发过群消息或交付卡
    let before_seq = service
        .store()
        .list_messages(group_id, 1, None)?
        .last()
        .map(|msg| msg.seq)
        .unwrap_or(0);

    // 4. 确保 Leader 稳定会话存在并构造专用工具表（排除重执行工具，专注 Router 编排）
    let session_id = ensure_leader_session(paths, &group)?;
    let leader_config = role_runtime_config(&base_config, true);
    let registry = build_leader_tool_registry(
        &leader_config,
        paths,
        Arc::clone(service),
        group_id.to_string(),
        leader.id.clone(),
        leader.name.clone(),
        active_thread_id.clone(),
        origin_channel.clone(),
        origin_target.clone(),
    )?;

    let batch_prompt = build_leader_batch_prompt(&triggers, &unsynthesized_reports);
    let system_prompt = leader_system_prompt(
        &group,
        &leader,
        &members,
        base_config.super_agent.im_reply_max_words,
    );
    let user_input = UserInputSubmission::new(batch_prompt, AgentMode::Yolo)
        .with_extra_system_prompt(system_prompt);
    let mut channel = crate::runner::ChannelSubmission::new("super_agent");
    for tool_name in super::tools::LEADER_TOOL_NAMES {
        channel = channel.with_extra_loaded_tool(*tool_name);
    }
    let submission = RunnerSubmission::user_input(SubmissionSource::Gateway, user_input)
        .with_session_id(session_id)
        .with_channel(channel);

    let workdir = resolve_group_workdir(&group.workspace_path, paths);
    let mut output = crate::runner::RunnerOutput::default();
    let stream_service = Arc::clone(service);
    let stream_group_id = group_id.to_string();
    let stream_leader_id = leader.id.clone();
    let run_result = crate::runtime_cwd::scope(workdir, async {
        let mut live_text = String::new();
        let mut sink = |event| {
            match &event {
                RunnerEvent::Agent(AgentEvent::ToolCall { name, .. })
                | RunnerEvent::Agent(AgentEvent::ToolCallIdentified { name, .. }) => {
                    let label = crate::tools::readable_tool_name(name);
                    let activity = format!("调用工具：{label}");
                    let _ = stream_service.store().update_member_state(
                        &stream_leader_id,
                        MemberStatus::Running,
                        Some(&activity),
                        None,
                    );
                    stream_service.emit_event(
                        "leader.activity",
                        Some(&stream_group_id),
                        json!({
                            "member_id": stream_leader_id,
                            "tool": name,
                            "activity": activity,
                        }),
                    );
                }
                RunnerEvent::Agent(AgentEvent::Chunk(chunk))
                    if chunk.kind == ChatStreamKind::Content && !chunk.text.is_empty() =>
                {
                    live_text.push_str(&chunk.text);
                    stream_service.emit_event(
                        "leader.delta",
                        Some(&stream_group_id),
                        json!({
                            "member_id": stream_leader_id,
                            "delta": chunk.text,
                            "preview": first_paragraph(&live_text, 320),
                        }),
                    );
                }
                _ => {}
            }
            output.push_event(event);
            Ok(())
        };
        SessionRunner::new(paths)
            .with_config(leader_config)
            .with_tool_registry(registry)
            .run_submission(submission, &mut sink)
            .await
    })
    .await;

    let _ = service
        .store()
        .update_member_state(&leader.id, MemberStatus::Idle, Some("ready"), None);

    let reply_text = match run_result {
        Ok(chat) => chat.map(|item| item.content).unwrap_or_default().trim().to_string(),
        Err(error) => {
            // 若来源于外部渠道，及时回推失败说明，避免微信/飞书/QQ 侧静默无响应
            if !matches!(origin_channel.as_str(), "" | "group" | "tui" | "web") {
                let notice = format!("Leader 编排执行失败：{error:#}");
                let _ = super::channel_bridge::deliver_to_channel(
                    paths,
                    service.store(),
                    group_id,
                    None,
                    &origin_channel,
                    &origin_target,
                    &notice,
                )
                .await;
            }
            service.emit_event(
                "leader.idle",
                Some(group_id),
                json!({ "member_id": leader.id }),
            );
            return Err(error);
        }
    };

    // 5. 标记本批 Trigger 已完成研判
    service.store().update_trigger_disposition(
        &trigger_ids,
        TriggerDisposition::Evaluated,
        Some("processed_by_leader"),
    )?;

    // 6. 检查 Leader 是否已显式调用 super_deliver_result / super_group_post
    let after_messages = service.store().list_messages(group_id, 50, Some(before_seq))?;
    let leader_delivered = after_messages
        .iter()
        .any(|msg| msg.sender_kind == MessageSenderKind::Leader && msg.kind == MessageKind::Delivery);
    let leader_posted = after_messages
        .iter()
        .any(|msg| msg.sender_kind == MessageSenderKind::Leader);

    // 7. 兜底保障：若存在未归纳的 Runner 报告且 Leader 直接用自然语言回复了总结，自动完成正式交付并推送回原渠道
    let remaining_reports: Vec<RunnerReport> = service
        .store()
        .list_unsynthesized_reports(Some(group_id))?
        .into_iter()
        .filter(|rep| {
            unsynthesized_reports
                .iter()
                .any(|initial| initial.id == rep.id)
        })
        .collect();

    if !remaining_reports.is_empty() && !leader_delivered && !reply_text.is_empty() {
        let report_ids: Vec<String> = remaining_reports.iter().map(|rep| rep.id.clone()).collect();
        let primary_task_id = remaining_reports
            .iter()
            .find_map(|rep| rep.task_id.clone());
        let summary = first_paragraph(&reply_text, 240);
        let _ = service
            .deliver_leader_result(
                group_id,
                &active_thread_id,
                primary_task_id.as_deref(),
                &report_ids,
                &summary,
                &reply_text,
                None,
                Some(origin_channel.as_str()),
                Some(&origin_target),
            )
            .await;
    } else if !leader_posted && !reply_text.is_empty() {
        // 普通对话回复写入群组时间线，并在来源于外部渠道时同步回推
        let _ = service.record_group_message(super::store::NewGroupMessage {
            group_id: group_id.to_string(),
            thread_id: active_thread_id.clone(),
            sender_kind: MessageSenderKind::Leader,
            sender_id: leader.id.clone(),
            sender_name: leader.name.clone(),
            recipient_id: None,
            recipient_name: None,
            kind: MessageKind::Chat,
            channel_source: (!matches!(origin_channel.as_str(), "group" | "tui" | "web"))
                .then_some(origin_channel.clone()),
            content: reply_text.clone(),
            summary: None,
            related_task_id: None,
            related_report_id: None,
        });
        if !matches!(origin_channel.as_str(), "" | "group" | "tui" | "web") {
            let _ = super::channel_bridge::deliver_to_channel(
                paths,
                service.store(),
                group_id,
                None,
                &origin_channel,
                &origin_target,
                &reply_text,
            )
            .await;
        }
    }

    service.emit_event(
        "leader.idle",
        Some(group_id),
        json!({ "member_id": leader.id }),
    );
    Ok(())
}

/// 组装提交给 Leader 的结构化批处理上下文（遵循 Inline & Spill 紧凑卡片规范）。
fn build_leader_batch_prompt(triggers: &[TriggerEvent], reports: &[RunnerReport]) -> String {
    let mut sections = Vec::new();
    if !triggers.is_empty() {
        let cards_json: Vec<Value> = triggers.iter().map(cards::trigger_card).collect();
        sections.push(format!(
            "<incoming-triggers count=\"{}\">\n{}\n</incoming-triggers>",
            triggers.len(),
            serde_json::to_string_pretty(&cards_json).unwrap_or_default()
        ));
    }
    if !reports.is_empty() {
        let cards_json: Vec<Value> = reports.iter().map(cards::report_card).collect();
        sections.push(format!(
            "<runner-reports count=\"{}\">\n{}\n</runner-reports>\n\
             以上是 Runner 提交的结构化工作报告。请按需调用 super_recall 阅读溢出细节，然后综合提炼成果，调用 super_deliver_result 主动向用户和来源渠道正式交付成果。",
            reports.len(),
            serde_json::to_string_pretty(&cards_json).unwrap_or_default()
        ));
    }
    sections.push(
        "请根据上述批次事件做出协调决策：若需具体开发、检索、排查或验证，请调用 super_spawn_runner 或 super_dispatch_task 异步派发给 Runner；若 Runner 已提交完成报告，请归纳后通过 super_deliver_result 向用户正式递交成果。"
            .to_string(),
    );
    sections.join("\n\n")
}

/// 解析本批次的来源渠道与回复目标。
fn resolve_batch_origin(
    triggers: &[TriggerEvent],
    reports: &[RunnerReport],
    service: &SuperAgentService,
) -> (String, Value) {
    if let Some(trigger) = triggers
        .iter()
        .rev()
        .find(|item| !matches!(item.source_channel.as_str(), "worker_report" | "watchdog"))
    {
        return (trigger.source_channel.clone(), trigger.metadata.clone());
    }
    for report in reports.iter().rev() {
        if let Some(task_id) = report.task_id.as_deref() {
            if let Ok(Some(task)) = service.store().get_task(task_id) {
                return (task.origin_channel, task.origin_target);
            }
        }
    }
    ("group".to_string(), json!({}))
}

/// 解析群组可用工作目录；若原目录已不存在则安全回退到当前目录或数据目录。
fn resolve_group_workdir(workspace_path: &str, paths: &SaiPaths) -> PathBuf {
    let trimmed = workspace_path.trim();
    if !trimmed.is_empty() {
        let candidate = PathBuf::from(trimmed);
        if candidate.is_dir() {
            return candidate;
        }
    }
    if let Ok(cwd) = crate::runtime_cwd::current_dir() {
        if cwd.is_dir() {
            return cwd;
        }
    }
    let fallback = paths.data_dir.join("workspace");
    let _ = std::fs::create_dir_all(&fallback);
    fallback
}

/// 提取回复首段作为简洁摘要。
fn first_paragraph(text: &str, max_chars: usize) -> String {
    let compact = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(3)
        .collect::<Vec<_>>()
        .join(" ");
    let mut chars = compact.chars();
    let head: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{head}...")
    } else {
        head
    }
}
