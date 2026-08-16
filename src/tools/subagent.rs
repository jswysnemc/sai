use super::subagent_runner::{ProgressMode, SubagentProgress, SubagentRunner, SubagentStats};
use super::{
    subagent_feed, subagent_runtime, subagent_state, ToolProgress, ToolRegistry, ToolSpec,
};
use crate::config::AppConfig;
use crate::i18n::text as t;
use crate::llm::OpenAiCompatibleClient;
use crate::paths::SaiPaths;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::time::Duration;

#[path = "subagent_args.rs"]
mod args;
mod control;
mod wait;

use args::{optional_string_arg, string_arg, summarize_prompt};
use control::{
    stats_json, subagent_cancel, subagent_list, subagent_result, subagent_send, subagent_status,
    subagent_stop,
};
use wait::wait_subagent;

const EXPLORE_PROMPT: &str = include_str!("../prompts/subagent-explore.md");
const GENERAL_PROMPT: &str = include_str!("../prompts/subagent-general.md");
/// 父代理显式传入 max_steps 时的上限；不传则不限制轮次。
///
/// 长程任务的工具轮数与任务规模相关，硬性上限会让子代理在半途被截断。
/// 收敛交给 wall-clock 超时与单工具超时，这里只兜住明显失控的显式取值。
const MAX_MAX_STEPS: usize = 400;
const SUBAGENT_TIMEOUT_SECONDS: u64 = 1800;
const TOOL_TIMEOUT_SECONDS: u64 = 120;
const DESCRIPTION_MAX_CHARS: usize = 160;
const EXPLORE_ALLOWED: &[&str] = &[
    "check_os_info",
    "read_file",
    "glob",
    "grep",
    "web_search",
    "web_fetch",
];

const GENERAL_EXCLUDED: &[&str] = &[
    "subagent",
    "background_command",
    "deep_diagnose",
    "linux_input_method_diagnose",
    "linux_game_compatibility",
    "set_alarm",
    "list_alarms",
    "cancel_alarm",
    "search_meme",
    "show_meme",
    "add_meme",
    "update_meme",
    "delete_meme",
    "generate_image",
    "search_web_images",
    "xuanxue_pick",
    "xuanxue_divine",
    "draw_zhouyi_hexagram",
    "draw_tarot_card",
    "draw_fortune_lot",
    "roll_dice",
];

#[derive(Clone)]
struct SubagentContext {
    config: AppConfig,
    paths: SaiPaths,
    tools: ToolRegistry,
    owner_key: String,
    session_id: String,
}

/// 注册交互式会话子智能体工具。
///
/// 参数:
/// - `registry`: 工具注册表
/// - `config`: 应用配置
/// - `paths`: Sai 路径
/// - `tools`: 子代理可用工具注册表
///
/// 返回:
/// - 无
pub(crate) fn register(
    registry: &mut ToolRegistry,
    config: AppConfig,
    paths: SaiPaths,
    tools: ToolRegistry,
    owner_key: String,
    session_id: String,
) {
    let context = SubagentContext {
        config,
        paths,
        tools,
        owner_key,
        session_id,
    };
    registry.register(ToolSpec::new_with_progress(
        "subagent",
        t(
            "Start and manage an in-process subagent. Only available in interactive REPL and Web sessions. action=start runs it in the background without blocking the conversation. Rules after start: do not interfere with a running subagent - never poll action=status in a loop, never redo or take over its task, and never cancel it unless the user asks. When a subagent finishes you receive an automatic system-reminder; then call action=result with its subagent_id (failed or cancelled runs carry the error there too). If you cannot proceed without the outcome, call action=wait to block until one finishes instead of polling. action=list shows all subagents; action=cancel stops one. Set persistent=true on start for a long-lived subagent: after finishing a task it stays idle instead of exiting, you get a system-reminder per finished segment, action=send appends follow-up messages to it (also works while it is running), and action=stop gracefully ends it (apply=false skips merging its worktree changes back).",
            "启动并管理进程内子智能体。此工具只在交互式 REPL 和 Web 会话中可用。action=start 在后台运行,不阻塞主对话。启动后的规约:不要干涉运行中的子智能体——不要循环调用 action=status 轮询,不要抢做或重做它的任务,除非用户要求也不要取消它。子智能体结束时你会收到自动的系统提醒,届时用 action=result 配合 subagent_id 取回结果(失败或取消的也在这里附带错误信息)。如果没有结果就无法继续,用 action=wait 阻塞等待完成,而不是轮询。action=list 列出全部子智能体;action=cancel 取消某个。启动时传 persistent=true 可创建持久子智能体:完成任务后进入待命而不退出,每完成一段你都会收到系统提醒,action=send 可追加消息(运行中也能发,步间注入),action=stop 优雅结束它(apply=false 跳过 worktree 变更合并)。",
        ),
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["start", "status", "result", "wait", "list", "cancel", "send", "stop"],
                    "description": t("Operation to perform. Defaults to start.", "要执行的操作，默认 start。")
                },
                "description": {
                    "type": "string",
                    "description": t("Short label for display when starting a subagent.", "启动子智能体时展示用的短描述。")
                },
                "prompt": {
                    "type": "string",
                    "description": t("Full instruction for the subagent.", "交给子智能体的完整指令。")
                },
                "subagent_type": {
                    "type": "string",
                    "description": format_subagent_profile_options(&context.config),
                },
                "max_steps": {
                    "type": "integer",
                    "description": t("Optional cap on the subagent's tool calls. Omit it to let the subagent run until the task is done.", "子代理工具调用次数上限，可选。不传表示不限制，子代理跑到任务完成为止。")
                },
                "subagent_id": {
                    "type": "string",
                    "description": t("Subagent id for status, result, cancel, wait, send, or stop. wait without it waits for any running subagent.", "status、result、cancel、wait、send 或 stop 使用的子智能体 ID。wait 不带它时等待任意一个运行中的子智能体。")
                },
                "timeout_seconds": {
                    "type": "integer",
                    "description": t("Max seconds for wait before returning. Defaults to 180, capped at 600.", "wait 的最长等待秒数，默认 180，上限 600。")
                },
                "persistent": {
                    "type": "boolean",
                    "description": t("For start: keep the subagent alive after it finishes a task so you can keep sending follow-ups with action=send; end it with action=stop. Defaults to false (one-shot).", "start 用:任务完成后保持存活待命,可用 action=send 继续追加消息,用 action=stop 结束。默认 false(一次性)。")
                },
                "message": {
                    "type": "string",
                    "description": t("For send: the follow-up message injected into the subagent's conversation at the next step boundary.", "send 用:追加给子智能体的消息,在其下一个步间间隙注入对话。")
                },
                "apply": {
                    "type": "boolean",
                    "description": t("For stop: whether to apply the subagent's worktree changes back to the parent workspace. Defaults to true.", "stop 用:结束时是否把子智能体的 worktree 变更合并回主工作区,默认 true。")
                }
            },
            "additionalProperties": false
        }),
        move |args, progress| {
            let context = context.clone();
            async move { run_subagent_action(args, context, progress).await }
        },
    ));
}

/// 生成主 Agent 可选择的子 Agent 描述列表。
///
/// 参数:
/// - `config`: 当前应用配置
///
/// 返回:
/// - 供模型理解档案用途的描述
fn format_subagent_profile_options(config: &AppConfig) -> String {
    let profiles = config
        .resolved_agent_profiles()
        .into_iter()
        .filter(|profile| profile.register_to_main)
        .map(|profile| format!("{}: {}", profile.id, profile.description))
        .collect::<Vec<_>>();
    format!("选择子 Agent 档案 id。{}", profiles.join("；"))
}

/// 分发子智能体操作。
///
/// 参数:
/// - `args`: 工具参数
/// - `context`: 子智能体上下文
/// - `progress`: 主对话工具进度上报器
///
/// 返回:
/// - JSON 字符串形式的操作结果
async fn run_subagent_action(
    args: Value,
    context: SubagentContext,
    progress: ToolProgress,
) -> Result<String> {
    let action = optional_string_arg(&args, "action")?.unwrap_or_else(|| "start".to_string());
    match action.as_str() {
        "start" => start_subagent(args, context.clone()).await,
        "status" => subagent_status(args, &context.owner_key),
        "result" => subagent_result(args, &context.owner_key),
        "wait" => wait_subagent(args, progress, &context.owner_key).await,
        "list" => subagent_list(&context.owner_key),
        "cancel" => subagent_cancel(args, &context.owner_key),
        "send" => subagent_send(args, &context.owner_key),
        "stop" => subagent_stop(args, &context.owner_key),
        _ => bail!("unsupported subagent action: {action}"),
    }
}

/// 启动后台子智能体。
///
/// 参数:
/// - `args`: 启动参数
/// - `context`: 子智能体上下文
///
/// 返回:
/// - 已创建子智能体的快照
async fn start_subagent(args: Value, context: SubagentContext) -> Result<String> {
    let prompt = string_arg(&args, "prompt")?;
    let requested_type = optional_string_arg(&args, "subagent_type")?;
    let profile = context
        .config
        .resolve_registered_agent(requested_type.as_deref())
        .with_context(|| "requested subagent is not exposed or does not exist")?;
    let description = optional_string_arg(&args, "description")?
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            if profile.description.trim().is_empty() {
                summarize_prompt(&prompt)
            } else {
                profile.description.clone()
            }
        });
    let subagent_type = profile.id.clone();
    // 不传 max_steps 时为 0，表示不限制工具轮次；显式传值才钳制到安全区间
    let max_steps = args
        .get("max_steps")
        .and_then(Value::as_u64)
        .map(|value| (value as usize).clamp(1, MAX_MAX_STEPS))
        .unwrap_or(0);
    // 持久子智能体：任务段完成后待命等待追加消息,直到显式 stop/cancel
    let persistent = args
        .get("persistent")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let goal_id = crate::state::StateStore::for_session(&context.paths, &context.session_id)?
        .goal()?
        .filter(|goal| goal.status.accepts_external_wake())
        .map(|goal| goal.id);
    let (subagent, cancel_rx) = subagent_state::create_subagent_for_owner_goal(
        &context.owner_key,
        goal_id,
        description,
        subagent_type,
        max_steps,
        persistent,
    );
    let _ =
        subagent_runtime::record_subagent_started(&context.paths, &context.session_id, &subagent);
    let subagent_id = subagent.id.clone();
    let parent_workdir =
        crate::runtime_cwd::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let isolated =
        match super::subagent_worktree::try_create(&parent_workdir, &subagent.description) {
            Ok(value) => value,
            Err(err) => {
                // Isolation is best-effort; fall back to parent workdir if git worktree fails.
                tracing_or_ignore(&format!("subagent worktree create failed: {err}"));
                None
            }
        };
    let runtime_cwd = if let Some(ref worktree) = isolated {
        subagent_state::set_subagent_worktree(
            &subagent_id,
            Some(worktree.worktree_root.display().to_string()),
            Some(worktree.branch_name.clone()),
            Some(worktree.parent_workdir.display().to_string()),
        );
        worktree.workdir.clone()
    } else {
        parent_workdir
    };
    tokio::spawn(async move {
        crate::runtime_cwd::scope(
            runtime_cwd,
            execute_subagent(subagent_id, prompt, context, cancel_rx, isolated),
        )
        .await;
    });
    let message = if persistent {
        t(
            "persistent subagent started; it stays idle after each finished task segment. A system-reminder arrives per segment; use action=result to read it, action=send to add follow-ups, action=stop to end it (worktree changes merge on stop by default)",
            "持久子智能体已启动;每完成一个任务段会进入待命并发送系统提醒。用 action=result 读取结果,action=send 追加消息,action=stop 结束(默认在 stop 时合并 worktree 变更)"
        )
    } else {
        t(
            "subagent started; continue your own work or call action=wait if you need the result. Do not poll action=status: a system-reminder arrives when it finishes",
            "子智能体已启动；请继续自己的工作,需要结果时用 action=wait 等待。不要轮询 action=status:完成时会收到系统提醒"
        )
    };
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "subagent": subagent,
        "message": message
    }))?)
}

/// 执行后台子智能体并写回状态。
///
/// 参数:
/// - `subagent_id`: 子智能体 ID
/// - `prompt`: 子智能体提示
/// - `context`: 子智能体上下文
/// - `cancel_rx`: 取消信号接收器
///
/// 返回:
/// - 无
async fn execute_subagent(
    subagent_id: String,
    prompt: String,
    context: SubagentContext,
    mut cancel_rx: tokio::sync::oneshot::Receiver<()>,
    worktree: Option<super::subagent_worktree::SubagentWorktree>,
) {
    let paths = context.paths.clone();
    let session_id = context.session_id.clone();
    let subagent = match subagent_state::subagent_snapshot(&subagent_id) {
        Ok(subagent) => subagent,
        Err(err) => {
            subagent_state::finish_subagent(
                &subagent_id,
                "failed",
                None,
                Some(err.to_string()),
                None,
            );
            record_finished_runtime_subagent(&paths, &session_id, &subagent_id);
            return;
        }
    };
    // 1. 起进度 channel，进度消息由 feed 解析写入时间线与快照，供前端实时渲染
    let (progress_tx, progress_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let mut progress_task = tokio::spawn(subagent_feed::consume_progress(
        subagent_id.clone(),
        progress_rx,
    ));
    let progress = ToolProgress::new(progress_tx);
    let result = tokio::select! {
        _ = &mut cancel_rx => Err(anyhow::anyhow!("cancelled")),
        result = run_subagent_session(&subagent, &prompt, context, progress) => result,
    };
    // 2. 运行结束会释放全部 sender，等待消费任务排空尾部流事件
    if tokio::time::timeout(Duration::from_secs(2), &mut progress_task)
        .await
        .is_err()
    {
        progress_task.abort();
    }
    // 3. 持久子智能体经 stop 结束时尊重其 apply 标志;一次性子智能体沿用成功即合并
    let apply_allowed = subagent_state::subagent_stop_requested(&subagent_id)
        .map(|stop| stop.apply)
        .unwrap_or(true);
    let merge_summary = finalize_worktree(
        &subagent_id,
        worktree.as_ref(),
        matches!(&result, Ok(_)) && apply_allowed,
    );
    match result {
        Ok((content, stats)) => {
            let content = append_merge_summary(content, merge_summary.as_ref());
            subagent_state::finish_subagent(
                &subagent_id,
                "completed",
                Some(content),
                None,
                Some(stats),
            );
            record_finished_runtime_subagent(&paths, &session_id, &subagent_id);
        }
        Err(err) if err.to_string() == "cancelled" => {
            subagent_state::finish_subagent(
                &subagent_id,
                "cancelled",
                None,
                Some("cancelled".to_string()),
                None,
            );
            record_finished_runtime_subagent(&paths, &session_id, &subagent_id);
        }
        Err(err) => {
            subagent_state::finish_subagent(
                &subagent_id,
                "failed",
                None,
                Some(err.to_string()),
                None,
            );
            record_finished_runtime_subagent(&paths, &session_id, &subagent_id);
        }
    }
}

/// Apply (on success) and always clean up a delegated worktree.
fn finalize_worktree(
    subagent_id: &str,
    worktree: Option<&super::subagent_worktree::SubagentWorktree>,
    completed_ok: bool,
) -> Option<serde_json::Value> {
    let worktree = worktree?;
    let mut summary = serde_json::Map::new();
    summary.insert(
        "worktree_root".to_string(),
        json!(worktree.worktree_root.display().to_string()),
    );
    summary.insert("branch".to_string(), json!(worktree.branch_name.clone()));

    if completed_ok {
        match super::subagent_worktree::apply(worktree) {
            Ok(apply_result) => {
                summary.insert(
                    "apply".to_string(),
                    serde_json::to_value(&apply_result).unwrap_or(json!({})),
                );
            }
            Err(err) => {
                summary.insert("apply_error".to_string(), json!(err.to_string()));
            }
        }
    } else {
        summary.insert("apply_skipped".to_string(), json!("subagent_not_completed"));
    }

    let cleanup = super::subagent_worktree::cleanup(worktree);
    summary.insert(
        "cleanup".to_string(),
        serde_json::to_value(&cleanup).unwrap_or(json!({})),
    );
    let value = Value::Object(summary);
    subagent_state::set_subagent_worktree_merge(subagent_id, value.clone());
    Some(value)
}

fn append_merge_summary(content: String, merge: Option<&Value>) -> String {
    let Some(merge) = merge else {
        return content;
    };
    let applied = merge
        .pointer("/apply/applied")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let changed = merge
        .pointer("/apply/changed")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let method = merge
        .pointer("/apply/apply_method")
        .and_then(Value::as_str)
        .unwrap_or("-");
    let apply_error = merge.get("apply_error").and_then(Value::as_str);
    let note = if let Some(error) = apply_error {
        format!(
            "

[worktree merge failed: {error}]"
        )
    } else if changed && applied {
        format!(
            "

[worktree changes auto-merged via {method}]"
        )
    } else if changed {
        "

[worktree had changes but auto-merge skipped or no-op]"
            .to_string()
    } else {
        String::new()
    };
    format!("{content}{note}")
}

fn tracing_or_ignore(message: &str) {
    // Avoid hard dependency on a tracing facade for this best-effort path.
    let _ = message;
}

/// 记录已结束子智能体的运行时状态。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `subagent_id`: 子智能体 ID
///
/// 返回:
/// - 无
fn record_finished_runtime_subagent(paths: &SaiPaths, session_id: &str, subagent_id: &str) {
    if let Ok(subagent) = subagent_state::subagent_snapshot(subagent_id) {
        let _ = subagent_runtime::record_subagent_finished(paths, session_id, &subagent);
    }
}

/// 按子智能体的持久标记分发到一次性或持久会话执行路径。
///
/// 参数:
/// - `subagent`: 子智能体启动快照
/// - `prompt`: 子智能体提示
/// - `context`: 子智能体上下文
/// - `tool_progress`: 写回快照的进度上报通道
///
/// 返回:
/// - 子代理输出内容和公开统计信息
async fn run_subagent_session(
    subagent: &subagent_state::SubagentSnapshot,
    prompt: &str,
    context: SubagentContext,
    tool_progress: ToolProgress,
) -> Result<(String, Value)> {
    if subagent.persistent {
        run_persistent_subagent(
            &subagent.id,
            &subagent.subagent_type,
            subagent.max_steps,
            prompt,
            context,
            tool_progress,
        )
        .await
    } else {
        run_subagent(
            &subagent.id,
            &subagent.subagent_type,
            subagent.max_steps,
            prompt,
            context,
            tool_progress,
        )
        .await
    }
}

/// 构造子代理执行器（模型客户端、工具集、系统提示与消息注入回调）。
///
/// 参数:
/// - `subagent_id`: 子智能体 ID，用于步间轮询其消息队列
/// - `subagent_type`: 子代理类型
/// - `max_steps`: 每个任务段的最大工具调用次数
/// - `context`: 子智能体上下文
/// - `tool_progress`: 写回快照的进度上报通道
/// - `persistent`: 是否为持久会话（持久会话不设整体运行时限提醒）
///
/// 返回:
/// - 可直接运行的子代理执行器
fn build_subagent_runner(
    subagent_id: &str,
    subagent_type: &str,
    max_steps: usize,
    context: &SubagentContext,
    tool_progress: ToolProgress,
    persistent: bool,
) -> Result<SubagentRunner> {
    // 1. 按子智能体模型配置构造客户端,未配置时沿用主对话供应商与模型
    let profile = context
        .config
        .resolve_registered_agent(Some(subagent_type))
        .with_context(|| format!("subagent profile is not exposed: {subagent_type}"))?;
    let client = build_subagent_client(context, &profile)?;
    let (default_prompt, default_tools, excluded) = match subagent_type {
        "explore" => (
            EXPLORE_PROMPT,
            context.tools.clone_filtered(EXPLORE_ALLOWED),
            Vec::new(),
        ),
        "general" => (
            GENERAL_PROMPT,
            context.tools.clone(),
            GENERAL_EXCLUDED.to_vec(),
        ),
        _ => (
            GENERAL_PROMPT,
            context.tools.clone(),
            GENERAL_EXCLUDED.to_vec(),
        ),
    };
    let tools = if inherits_default_tools(&context.config, &profile) {
        default_tools
    } else {
        let allowed = profile
            .enabled_tools
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        default_tools.clone_filtered(&allowed)
    };
    // 2. 渐进网关按子 Agent 的实际工具集合和延迟配置重建，避免沿用主 Agent 的描述
    let mut tools = tools.clone_excluding(&[super::LOAD_NAME, super::INVOKE_NAME]);
    super::progressive::register_loader(&mut tools, &profile.deferred_tools);
    let base_prompt = if profile.system_prompt.trim().is_empty() {
        default_prompt.to_string()
    } else {
        profile.system_prompt.clone()
    };
    let system_prompt = subagent_system_prompt(context, &profile, &base_prompt)?;
    // 3. 以 Full 模式上报,时间线可拿到工具调用参数、结果与流式文本
    let progress = SubagentProgress::new(tool_progress, ProgressMode::Full, true);
    // 4. 步间消息注入:主代理 action=send 与用户留言经消息队列在此进入对话
    let poll_id = subagent_id.to_string();
    let message_poll: super::subagent_runner::SubagentMessagePoll =
        std::sync::Arc::new(move || {
            subagent_state::drain_subagent_inbox(&poll_id)
                .into_iter()
                .map(|message| super::subagent_runner::SubagentInjectedMessage {
                    from: message.from,
                    text: message.text,
                })
                .collect()
        });
    Ok(SubagentRunner::new(client, &system_prompt, tools, progress)
        .progressive_loading(
            profile.deferred_tools.clone(),
            context.config.clone(),
            context.paths.clone(),
        )
        .max_steps(max_steps)
        .timeout_seconds(TOOL_TIMEOUT_SECONDS)
        // 持久会话面向多任务段长期存活,不注入整体时限收尾提醒
        .session_deadline_seconds(if persistent {
            0
        } else {
            SUBAGENT_TIMEOUT_SECONDS
        })
        .excluded_tools(&excluded)
        .with_message_poll(message_poll))
}

/// 运行指定类型的子代理。
///
/// 参数:
/// - `subagent_id`: 子智能体 ID
/// - `subagent_type`: 子代理类型
/// - `max_steps`: 最大工具调用次数
/// - `prompt`: 子智能体提示
/// - `context`: 子智能体上下文
/// - `tool_progress`: 写回快照的进度上报通道
///
/// 返回:
/// - 子代理输出内容和公开统计信息
async fn run_subagent(
    subagent_id: &str,
    subagent_type: &str,
    max_steps: usize,
    prompt: &str,
    context: SubagentContext,
    tool_progress: ToolProgress,
) -> Result<(String, Value)> {
    let runner = build_subagent_runner(
        subagent_id,
        subagent_type,
        max_steps,
        &context,
        tool_progress,
        false,
    )?;
    let result = tokio::time::timeout(
        Duration::from_secs(SUBAGENT_TIMEOUT_SECONDS),
        runner.run(prompt),
    )
    .await
    .map_err(|_| anyhow::anyhow!("subagent timed out after {SUBAGENT_TIMEOUT_SECONDS}s"))??;
    let (chat_result, stats) = result;
    if chat_result.content.trim().is_empty() {
        bail!("subagent returned an empty result");
    }
    Ok((chat_result.content, stats_json(&stats)))
}

/// 持久子智能体待命轮询间隔（毫秒）。
const IDLE_POLL_MILLIS: u64 = 300;

/// 运行持久子智能体的多任务段会话循环。
///
/// 生命周期:running（执行任务段）与 idle（待命等新消息）交替，
/// 直到收到 stop 请求（段间生效，不打断段内工具调用）或被取消。
/// 对话历史与渐进加载状态跨任务段共享，统计信息累计。
///
/// 参数:
/// - `subagent_id`: 子智能体 ID
/// - `subagent_type`: 子代理类型
/// - `max_steps`: 每个任务段的最大工具调用次数
/// - `prompt`: 首个任务段的提示
/// - `context`: 子智能体上下文
/// - `tool_progress`: 写回快照的进度上报通道
///
/// 返回:
/// - 最后一个任务段的输出内容和累计统计信息
async fn run_persistent_subagent(
    subagent_id: &str,
    subagent_type: &str,
    max_steps: usize,
    prompt: &str,
    context: SubagentContext,
    tool_progress: ToolProgress,
) -> Result<(String, Value)> {
    let runner = build_subagent_runner(
        subagent_id,
        subagent_type,
        max_steps,
        &context,
        tool_progress,
        true,
    )?;
    let mut messages = runner.initial_messages(prompt);
    let mut tool_visibility = runner.fresh_tool_visibility();
    let mut stats = SubagentStats::default();
    loop {
        // 1. 跑一个任务段;段级超时兜住单段挂死,超时按失败处理
        let result = tokio::time::timeout(
            Duration::from_secs(SUBAGENT_TIMEOUT_SECONDS),
            runner.run_turn(&mut messages, &mut stats, &mut tool_visibility),
        )
        .await
        .map_err(|_| {
            anyhow::anyhow!("subagent segment timed out after {SUBAGENT_TIMEOUT_SECONDS}s")
        })??;
        let content = if result.content.trim().is_empty() {
            t(
                "(no text output for this segment)",
                "（本任务段无正文输出）",
            )
            .to_string()
        } else {
            result.content
        };
        let stats_value = stats_json(&stats);
        // 2. 段间检查结束请求:stop 不打断段内执行,在段完成后生效
        if subagent_state::subagent_stop_requested(subagent_id).is_some() {
            return Ok((content, stats_value));
        }
        // 3. 转入待命;转入失败说明已被取消等,直接交回当前段结果
        if !subagent_state::park_subagent(
            subagent_id,
            Some(content.clone()),
            Some(stats_value.clone()),
        ) {
            return Ok((content, stats_value));
        }
        // 4. 待命循环:等待追加消息或结束请求;取消由外层 select 直接中止
        loop {
            if subagent_state::subagent_stop_requested(subagent_id).is_some() {
                return Ok((content, stats_value));
            }
            if subagent_state::subagent_inbox_len(subagent_id) > 0 {
                subagent_state::resume_subagent(subagent_id);
                break;
            }
            tokio::time::sleep(Duration::from_millis(IDLE_POLL_MILLIS)).await;
        }
    }
}

/// 判断子 Agent 是否应沿用类型内置的工具集合。
///
/// 参数:
/// - `config`: 当前应用配置
/// - `profile`: 已解析的统一 Agent 档案
///
/// 返回:
/// - 内置 Agent 或旧版迁移档案在工具为空时返回 true
fn inherits_default_tools(config: &AppConfig, profile: &crate::config::AgentProfile) -> bool {
    profile.enabled_tools.is_empty()
        && (matches!(profile.id.as_str(), "general" | "explore")
            || !config.agents.iter().any(|agent| agent.id == profile.id))
}

/// 按子智能体模型配置构造 LLM 客户端。
///
/// 子智能体配置了独立供应商/模型时,在一份克隆配置上覆盖 active_provider 与该供应商
/// 的 default_model;未配置时直接沿用主对话配置。
///
/// 参数:
/// - `context`: 子智能体上下文
///
/// 返回:
/// - LLM 客户端
fn build_subagent_client(
    context: &SubagentContext,
    profile: &crate::config::AgentProfile,
) -> Result<OpenAiCompatibleClient> {
    let subagent = &context.config.subagent;
    // 1. 未配置任何子智能体供应商与模型,沿用主对话配置
    if subagent.provider_id.is_empty()
        && subagent.model.is_empty()
        && profile.provider_id.is_empty()
        && profile.model.is_empty()
        && (profile.thinking_level.is_empty() || profile.thinking_level == "auto")
    {
        return OpenAiCompatibleClient::from_config(&context.config, &context.paths);
    }
    let mut config = context.config.clone();
    // 2. 指定了供应商则切换 active_provider,否则在当前供应商上改模型
    let provider_id = if profile.provider_id.is_empty() {
        &subagent.provider_id
    } else {
        &profile.provider_id
    };
    if !provider_id.is_empty() {
        config.active_provider = provider_id.clone();
    }
    let active = config.active_provider.clone();
    if let Some(provider) = config
        .providers
        .iter_mut()
        .find(|provider| provider.id == active)
    {
        let model = if profile.model.is_empty() {
            &subagent.model
        } else {
            &profile.model
        };
        if !model.is_empty() {
            provider.default_model = model.clone();
        }
        let thinking = if profile.thinking_level.is_empty() || profile.thinking_level == "auto" {
            &subagent.thinking_level
        } else {
            &profile.thinking_level
        };
        if !thinking.is_empty() && thinking != "auto" {
            provider.thinking_level = thinking.clone();
        }
    }
    OpenAiCompatibleClient::from_config(&config, &context.paths)
}

/// 组合 Agent 系统提示词与该档案启用的 Skills。
///
/// 参数:
/// - `context`: 子智能体运行上下文
/// - `profile`: 统一 Agent 档案
/// - `base_prompt`: Agent 基础系统提示词
///
/// 返回:
/// - 可直接交给子智能体的完整系统提示词
fn subagent_system_prompt(
    context: &SubagentContext,
    profile: &crate::config::AgentProfile,
    base_prompt: &str,
) -> Result<String> {
    if profile.skills_full.is_empty() && profile.skills_named.is_empty() {
        return Ok(base_prompt.to_string());
    }
    let mut config = context.config.clone();
    config.agent_runtime = Some(crate::config::AgentRuntimeOverride {
        enabled_tools: profile.enabled_tools.clone(),
        exclusive: profile.tools_exclusive,
        deferred_tools: profile.deferred_tools.clone(),
        skills_full: profile.skills_full.clone(),
        skills_named: profile.skills_named.clone(),
    });
    let skills = crate::tools::skills_prompt(&config, &context.paths)?;
    if skills.trim().is_empty() {
        Ok(base_prompt.to_string())
    } else {
        Ok(format!("{base_prompt}\n\n{skills}"))
    }
}

include!("subagent_tests.rs");
