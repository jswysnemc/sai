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

use args::{optional_string_arg, string_arg, summarize_prompt};

const EXPLORE_PROMPT: &str = include_str!("../prompts/subagent-explore.md");
const GENERAL_PROMPT: &str = include_str!("../prompts/subagent-general.md");
const DEFAULT_MAX_STEPS: usize = 20;
const MAX_MAX_STEPS: usize = 80;
const SUBAGENT_TIMEOUT_SECONDS: u64 = 1800;
const TOOL_TIMEOUT_SECONDS: u64 = 120;
const DESCRIPTION_MAX_CHARS: usize = 160;
const WAIT_DEFAULT_SECONDS: u64 = 180;
const WAIT_MAX_SECONDS: u64 = 600;
const WAIT_POLL_MILLIS: u64 = 500;
const WAIT_REPORT_EVERY_SECONDS: u64 = 5;

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
    "deep_research",
    "deep_diagnose",
    "linux_input_method_diagnose",
    "linux_game_compatibility",
    "load",
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
            "Start and manage an in-process subagent. Only available in interactive REPL and Web sessions. action=start runs it in the background without blocking the conversation. Rules after start: do not interfere with a running subagent - never poll action=status in a loop, never redo or take over its task, and never cancel it unless the user asks. When a subagent finishes you receive an automatic system-reminder; then call action=result with its subagent_id (failed or cancelled runs carry the error there too). If you cannot proceed without the outcome, call action=wait to block until one finishes instead of polling. action=list shows all subagents; action=cancel stops one.",
            "启动并管理进程内子智能体。此工具只在交互式 REPL 和 Web 会话中可用。action=start 在后台运行,不阻塞主对话。启动后的规约:不要干涉运行中的子智能体——不要循环调用 action=status 轮询,不要抢做或重做它的任务,除非用户要求也不要取消它。子智能体结束时你会收到自动的系统提醒,届时用 action=result 配合 subagent_id 取回结果(失败或取消的也在这里附带错误信息)。如果没有结果就无法继续,用 action=wait 阻塞等待完成,而不是轮询。action=list 列出全部子智能体;action=cancel 取消某个。",
        ),
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["start", "status", "result", "wait", "list", "cancel"],
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
                    "description": t("Maximum tool calls for the subagent. Defaults to 20.", "子代理最大工具调用次数，默认 20。")
                },
                "subagent_id": {
                    "type": "string",
                    "description": t("Subagent id for status, result, cancel, or wait. wait without it waits for any running subagent.", "status、result、cancel 或 wait 使用的子智能体 ID。wait 不带它时等待任意一个运行中的子智能体。")
                },
                "timeout_seconds": {
                    "type": "integer",
                    "description": t("Max seconds for wait before returning. Defaults to 180, capped at 600.", "wait 的最长等待秒数，默认 180，上限 600。")
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
    let max_steps = args
        .get("max_steps")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or(DEFAULT_MAX_STEPS)
        .clamp(1, MAX_MAX_STEPS);
    let (subagent, cancel_rx) = subagent_state::create_subagent_for_owner(
        &context.owner_key,
        description,
        subagent_type,
        max_steps,
    );
    let _ =
        subagent_runtime::record_subagent_started(&context.paths, &context.session_id, &subagent);
    let subagent_id = subagent.id.clone();
    let runtime_cwd =
        crate::runtime_cwd::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    tokio::spawn(async move {
        crate::runtime_cwd::scope(
            runtime_cwd,
            execute_subagent(subagent_id, prompt, context, cancel_rx),
        )
        .await;
    });
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "subagent": subagent,
        "message": t(
            "subagent started; continue your own work or call action=wait if you need the result. Do not poll action=status: a system-reminder arrives when it finishes",
            "子智能体已启动；请继续自己的工作,需要结果时用 action=wait 等待。不要轮询 action=status:完成时会收到系统提醒"
        )
    }))?)
}

/// 阻塞等待子智能体进入终态。
///
/// 指定 subagent_id 时等待该子智能体;未指定时等待任意一个新完成的子智能体。
/// 返回结果后仍由主模型请求确认投递，避免请求失败时丢失通知。
///
/// 参数:
/// - `args`: 等待参数
/// - `progress`: 主对话工具进度上报器
/// - `owner_key`: 父会话稳定作用域键
///
/// 返回:
/// - 完成子智能体的快照,或超时说明
async fn wait_subagent(args: Value, progress: ToolProgress, owner_key: &str) -> Result<String> {
    let subagent_id = optional_string_arg(&args, "subagent_id")?.filter(|id| !id.is_empty());
    let timeout = args
        .get("timeout_seconds")
        .and_then(Value::as_u64)
        .unwrap_or(WAIT_DEFAULT_SECONDS)
        .clamp(5, WAIT_MAX_SECONDS);
    let started = tokio::time::Instant::now();
    let mut last_report = 0u64;
    loop {
        // 1. 指定 id:该子智能体进入终态即返回
        if let Some(id) = &subagent_id {
            let snapshot = subagent_state::subagent_snapshot_for_owner(owner_key, id)?;
            if snapshot.status != "running" {
                return Ok(serde_json::to_string_pretty(&json!({
                    "ok": snapshot.status == "completed",
                    "subagent": snapshot
                }))?);
            }
        } else {
            // 2. 未指定 id:任意新完成的子智能体即返回,并消费其通知事件
            let notices = subagent_state::pending_finished_notices(owner_key);
            if !notices.is_empty() {
                let finished = notices
                    .iter()
                    .filter_map(|notice| subagent_state::subagent_snapshot(&notice.id).ok())
                    .collect::<Vec<_>>();
                return Ok(serde_json::to_string_pretty(&json!({
                    "ok": true,
                    "finished": finished
                }))?);
            }
            let subagents = subagent_state::list_subagents_for_owner(owner_key);
            if subagents
                .iter()
                .all(|snapshot| snapshot.status != "running")
            {
                return Ok(serde_json::to_string_pretty(&json!({
                    "ok": true,
                    "message": t(
                        "no running subagents to wait for; results were already delivered",
                        "没有运行中的子智能体可等待,结果此前已经送达"
                    ),
                    "subagents": subagents
                }))?);
            }
        }
        let elapsed = started.elapsed().as_secs();
        if elapsed >= timeout {
            return Ok(serde_json::to_string_pretty(&json!({
                "ok": false,
                "timeout": true,
                "message": t(
                    "wait timed out while subagents are still running; continue other work, a system-reminder arrives on finish",
                    "等待超时,子智能体仍在运行;请先继续其他工作,完成时会收到系统提醒"
                )
            }))?);
        }
        // 3. 周期性上报等待进度,避免前端看起来卡死
        if elapsed >= last_report + WAIT_REPORT_EVERY_SECONDS {
            last_report = elapsed;
            progress.report(if crate::i18n::is_zh() {
                format!("等待子智能体完成,已等待 {elapsed} 秒")
            } else {
                format!("waiting for subagent, {elapsed}s elapsed")
            });
        }
        tokio::time::sleep(Duration::from_millis(WAIT_POLL_MILLIS)).await;
    }
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
        result = run_subagent(
            &subagent.subagent_type,
            subagent.max_steps,
            &prompt,
            context,
            progress,
        ) => result,
    };
    // 2. 运行结束会释放全部 sender，等待消费任务排空尾部流事件
    if tokio::time::timeout(Duration::from_secs(2), &mut progress_task)
        .await
        .is_err()
    {
        progress_task.abort();
    }
    match result {
        Ok((content, stats)) => {
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

/// 运行指定类型的子代理。
///
/// 参数:
/// - `subagent_type`: 子代理类型
/// - `max_steps`: 最大工具调用次数
/// - `prompt`: 子智能体提示
/// - `context`: 子智能体上下文
/// - `tool_progress`: 写回快照的进度上报通道
///
/// 返回:
/// - 子代理输出内容和公开统计信息
async fn run_subagent(
    subagent_type: &str,
    max_steps: usize,
    prompt: &str,
    context: SubagentContext,
    tool_progress: ToolProgress,
) -> Result<(String, Value)> {
    // 1. 按子智能体模型配置构造客户端,未配置时沿用主对话供应商与模型
    let profile = context
        .config
        .resolve_registered_agent(Some(subagent_type))
        .with_context(|| format!("subagent profile is not exposed: {subagent_type}"))?;
    let client = build_subagent_client(&context, &profile)?;
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
    let base_prompt = if profile.system_prompt.trim().is_empty() {
        default_prompt.to_string()
    } else {
        profile.system_prompt.clone()
    };
    let system_prompt = subagent_system_prompt(&context, &profile, &base_prompt)?;
    // 2. 以 Full 模式上报,时间线可拿到工具调用参数、结果与流式文本
    let progress = SubagentProgress::new(tool_progress, ProgressMode::Full, true);
    let runner = SubagentRunner::new(client, &system_prompt, tools, progress)
        .max_steps(max_steps)
        .timeout_seconds(TOOL_TIMEOUT_SECONDS)
        .excluded_tools(&excluded);
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

/// 生成子智能体统计 JSON。
///
/// 参数:
/// - `stats`: 子代理统计
///
/// 返回:
/// - 公开统计信息
fn stats_json(stats: &SubagentStats) -> Value {
    let mut value = stats.public();
    if let Value::Object(map) = &mut value {
        map.insert("budget_reached".to_string(), json!(stats.budget_reached));
    }
    value
}

/// 查询单个后台子智能体状态。
///
/// 参数:
/// - `args`: 查询参数
///
/// 返回:
/// - 子智能体快照
fn subagent_status(args: Value, owner_key: &str) -> Result<String> {
    let subagent_id = string_arg(&args, "subagent_id")?;
    let subagent = subagent_state::subagent_snapshot_for_owner(owner_key, &subagent_id)?;
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "subagent": subagent
    }))?)
}

/// 查询后台子智能体结果。
///
/// 参数:
/// - `args`: 查询参数
///
/// 返回:
/// - 子智能体结果或当前状态
fn subagent_result(args: Value, owner_key: &str) -> Result<String> {
    let subagent_id = string_arg(&args, "subagent_id")?;
    let subagent = subagent_state::subagent_snapshot_for_owner(owner_key, &subagent_id)?;
    Ok(serde_json::to_string_pretty(&json!({
        "ok": subagent.status == "completed",
        "subagent": subagent
    }))?)
}

/// 列出后台子智能体。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 子智能体列表
fn subagent_list(owner_key: &str) -> Result<String> {
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "subagents": subagent_state::list_subagents_for_owner(owner_key)
    }))?)
}

/// 取消后台子智能体。
///
/// 参数:
/// - `args`: 取消参数
///
/// 返回:
/// - 取消后的子智能体快照
fn subagent_cancel(args: Value, owner_key: &str) -> Result<String> {
    let subagent_id = string_arg(&args, "subagent_id")?;
    let subagent = subagent_state::cancel_subagent_for_owner(owner_key, &subagent_id)?;
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "subagent": subagent
    }))?)
}

include!("subagent_tests.rs");
