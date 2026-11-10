use super::subagent_runner::{ProgressMode, SubagentProgress, SubagentRunner};
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
mod execution;
mod recovery;
use execution::execute_subagent;
#[cfg(test)]
#[path = "subagent/model_settings_tests.rs"]
mod model_settings_tests;
mod runner_factory;
mod wait;

use runner_factory::build_subagent_runner;
#[cfg(test)]
use runner_factory::inherits_default_tools;

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
    "create_goal",
    "get_goal",
    "update_goal",
    "subagent",
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
                    "enum": ["start", "status", "result", "wait", "list", "cancel", "send", "stop", "resume"],
                    "description": t("Operation to perform. Defaults to start. Use resume to continue an interrupted subagent from its persisted conversation and original working directory.", "要执行的操作，默认 start。resume 从完整对话检查点和原工作目录继续中断的子任务。")
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
        "resume" => recovery::resume_interrupted(args, context).await,
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
async fn start_subagent(args: Value, mut context: SubagentContext) -> Result<String> {
    runner_factory::refresh_model_settings(&mut context)?;
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
    subagent_state::set_execution_context(
        &subagent_id,
        super::subagent_persistence::SubagentExecutionContext {
            cwd: runtime_cwd.clone(),
            worktree: isolated.clone(),
        },
    )?;
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

fn tracing_or_ignore(message: &str) {
    // Avoid hard dependency on a tracing facade for this best-effort path.
    let _ = message;
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
    let config = crate::config::subagent_runtime_config(&context.config, profile)?;
    OpenAiCompatibleClient::from_config(&config, &context.paths)
}

include!("subagent_tests.rs");
