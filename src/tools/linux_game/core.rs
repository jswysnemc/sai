use super::{readable_tool_name, ToolProgress, ToolRegistry, ToolSpec};
use crate::config::AppConfig;
use crate::i18n::is_zh;
use crate::llm::{
    ChatMessage, ChatResult, ChatStreamChunk, ChatStreamKind, OpenAiCompatibleClient, Usage,
};
use crate::paths::SaiPaths;
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::time::Duration;

const GAME_COMPATIBILITY_PROMPT: &str = crate::prompts::GAME_COMPATIBILITY_PROMPT;

const OUTPUT_INSTRUCTION: &str = r#"这是 Linux 游戏兼容性调查子代理返回的最终报告。

请把 final_report 当作主要依据回复用户。不要重新编造兼容性结论。

回复时保留以下核心信息：
- 红绿灯结论，能不能玩
- 怎么玩
- 注意事项

如果用户问“怎么玩”，必须给出可执行步骤。
如果用户追问“刚才完整报告”，直接复述 final_report。"#;

#[derive(Clone)]
struct GameCompatibilityContext {
    config: AppConfig,
    paths: SaiPaths,
    tools: ToolRegistry,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ProgressMode {
    Hidden,
    Summary,
    Full,
}

#[derive(Clone)]
struct GameProgress {
    progress: ToolProgress,
    mode: ProgressMode,
}

impl GameProgress {
    fn new(config: &AppConfig, progress: ToolProgress) -> Self {
        let mode = match config
            .display
            .tool_calls
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "hidden" => ProgressMode::Hidden,
            "full" => ProgressMode::Full,
            _ => ProgressMode::Summary,
        };
        Self { progress, mode }
    }

    fn report(&self, message: impl Into<String>) {
        self.progress.report(message);
    }

    fn subtool(&self, message: impl Into<String>) {
        if self.mode == ProgressMode::Full {
            self.progress.report(message);
        }
    }

    fn reasoning(&self, text: &str) {
        if self.mode != ProgressMode::Hidden {
            self.progress
                .report(format!("__subagent_reasoning__{}", text));
        }
    }
}

#[derive(Default)]
struct GameStats {
    tool_calls: usize,
    tool_ok: usize,
    tool_errors: usize,
    prompt_tokens: u64,
    completion_tokens: u64,
    total_tokens: u64,
    token_estimate: u64,
    token_estimate_method: TokenEstimateMethod,
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
enum TokenEstimateMethod {
    #[default]
    None,
    ProviderUsage,
    ProviderUsagePlusEstimate,
    RoughCharEstimate,
}

impl GameStats {
    fn add_usage_or_estimate(&mut self, usage: Option<&Usage>, texts: &[&str]) {
        if let Some(usage) = usage {
            if usage.total_tokens > 0 {
                self.prompt_tokens += usage.prompt_tokens;
                self.completion_tokens += usage.completion_tokens;
                self.total_tokens += usage.total_tokens;
                self.token_estimate += usage.total_tokens;
                self.token_estimate_method = match self.token_estimate_method {
                    TokenEstimateMethod::None | TokenEstimateMethod::ProviderUsage => {
                        TokenEstimateMethod::ProviderUsage
                    }
                    _ => TokenEstimateMethod::ProviderUsagePlusEstimate,
                };
                return;
            }
        }
        let estimate = estimate_tokens(texts);
        self.token_estimate += estimate;
        self.token_estimate_method = match self.token_estimate_method {
            TokenEstimateMethod::None | TokenEstimateMethod::RoughCharEstimate => {
                TokenEstimateMethod::RoughCharEstimate
            }
            _ => TokenEstimateMethod::ProviderUsagePlusEstimate,
        };
    }

    fn public(&self) -> Value {
        json!({
            "tool_calls": self.tool_calls,
            "tool_ok": self.tool_ok,
            "tool_errors": self.tool_errors,
            "prompt_tokens": self.prompt_tokens,
            "completion_tokens": self.completion_tokens,
            "total_tokens": self.total_tokens,
            "token_estimate": self.token_estimate,
            "token_estimate_method": token_estimate_method_label(self.token_estimate_method),
            "token_estimate_is_actual": self.token_estimate_method == TokenEstimateMethod::ProviderUsage,
        })
    }
}

fn token_estimate_method_label(method: TokenEstimateMethod) -> &'static str {
    match method {
        TokenEstimateMethod::ProviderUsage => "provider_usage",
        TokenEstimateMethod::ProviderUsagePlusEstimate => "provider_usage_plus_estimate",
        TokenEstimateMethod::RoughCharEstimate | TokenEstimateMethod::None => "rough_char_estimate",
    }
}

fn estimate_tokens(texts: &[&str]) -> u64 {
    crate::token_estimate::estimate_texts_tokens(texts)
}

pub fn register(
    registry: &mut ToolRegistry,
    config: AppConfig,
    paths: SaiPaths,
    tools: ToolRegistry,
) {
    let context = GameCompatibilityContext {
        config,
        paths,
        tools,
    };
    registry.register(ToolSpec::new_with_progress(
        "linux_game_compatibility",
        "Run the Linux game compatibility investigation sub-agent and return its final report. / 运行 Linux 游戏兼容性调查子代理并返回最终报告。",
        json!({"type":"object","properties":{"game":{"type":"string","description":"Game title. / 游戏名称。"},"issue":{"type":"string","description":"Optional issue such as crash, multiplayer, anti-cheat, performance, mods. / 可选关注点，例如崩溃、多人、反作弊、性能、Mod。"}},"required":["game"],"additionalProperties":false}),
        move |args, progress| {
            let context = context.clone();
            async move { linux_game_compatibility(args, context, progress).await }
        },
    ));
}

async fn linux_game_compatibility(
    args: Value,
    context: GameCompatibilityContext,
    progress: ToolProgress,
) -> Result<String> {
    let game = required(&args, "game")?;
    let issue = args
        .get("issue")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let progress = GameProgress::new(&context.config, progress);
    progress.report(format!("{}: {}", "Linux 游戏兼容性", game));
    let client = OpenAiCompatibleClient::from_config(&context.config, &context.paths)?;
    let system_prompt = GAME_COMPATIBILITY_PROMPT;
    let prompt = format!(
        "用户问题：\n游戏：{game}\n关注点：{}\n\n请按系统提示词流程完成调查。第一步必须调用 gather_linux_game_compatibility_signals。最终只输出调查报告。",
        if issue.trim().is_empty() { "未明确" } else { &issue }
    );
    let mut stats = GameStats::default();
    let result = chat_with_tools(
        &client,
        vec![
            ChatMessage::system(system_prompt),
            ChatMessage::plain("user", prompt.clone()),
        ],
        game_tool_registry(&context),
        context
            .config
            .plugins
            .linux_game_compatibility
            .max_tool_steps,
        &progress,
        &mut stats,
    )
    .await?;
    stats.add_usage_or_estimate(
        result.usage.as_ref(),
        &[system_prompt, &prompt, &result.content],
    );
    let report = strip_report_preamble(&result.content);
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "kind": "linux_game_compatibility",
        "game_query": game,
        "final_report": report,
        "stats": stats.public(),
        "output_instruction": OUTPUT_INSTRUCTION,
    }))?)
}

fn game_tool_registry(context: &GameCompatibilityContext) -> ToolRegistry {
    let mut registry = context.tools.clone();
    registry.register(ToolSpec::new(
        "gather_linux_game_compatibility_signals",
        "Gather Steam, ProtonDB, Can I Play on Linux, and AreWeAntiCheatYet compatibility signals for one game. / 收集单个游戏在 Steam、ProtonDB、Can I Play on Linux、AreWeAntiCheatYet 上的兼容性信号。",
        json!({"type":"object","properties":{"game":{"type":"string","description":"Game title. / 游戏名称。"},"issue":{"type":"string","description":"Optional issue such as crash, multiplayer, anti-cheat, performance, mods. / 可选关注点，例如崩溃、多人、反作弊、性能、Mod。"}},"required":["game"],"additionalProperties":false}),
        |args| async move { gather_linux_game_compatibility_signals(args).await },
    ));
    registry
}

async fn chat_with_tools(
    client: &OpenAiCompatibleClient,
    mut messages: Vec<ChatMessage>,
    tools: ToolRegistry,
    max_tool_steps: usize,
    progress: &GameProgress,
    stats: &mut GameStats,
) -> Result<ChatResult> {
    let definitions = tools.definitions_except(&["linux_game_compatibility", "deep_research"]);
    let mut steps = 0usize;
    loop {
        if max_tool_steps > 0 && steps >= max_tool_steps {
            messages.push(ChatMessage::plain("user", finalization_prompt()));
            let result = client
                .chat_stream(messages, Vec::new(), |chunk| {
                    if chunk.kind == ChatStreamKind::Reasoning {
                        progress.reasoning(&chunk.text);
                    }
                    Ok(())
                })
                .await?;
            stats.add_usage_or_estimate(result.usage.as_ref(), &[&result.content]);
            return Ok(result);
        }
        let result = client
            .chat_stream(
                messages.clone(),
                definitions.clone(),
                |chunk: ChatStreamChunk| {
                    if chunk.kind == ChatStreamKind::Reasoning {
                        progress.reasoning(&chunk.text);
                    }
                    Ok(())
                },
            )
            .await?;
        stats.add_usage_or_estimate(result.usage.as_ref(), &[]);
        if result.tool_calls.is_empty() {
            return Ok(result);
        }
        if !result.content.trim().is_empty() {
            messages.push(ChatMessage::assistant(result.content.clone(), None));
        }
        let mut transcript = Vec::new();
        for call in result.tool_calls {
            if max_tool_steps > 0 && steps >= max_tool_steps {
                transcript.push(render_internal_tool_result(
                    &call.function.name,
                    &call.function.arguments,
                    false,
                    "tool skipped: game compatibility tool budget reached",
                ));
                continue;
            }
            steps += 1;
            stats.tool_calls += 1;
            if progress.mode == ProgressMode::Summary {
                progress.report(if is_zh() {
                    format!(
                        "工具 #{steps}：{} 运行中",
                        readable_tool_name(&call.function.name)
                    )
                } else {
                    format!("tool #{steps}: {} running", call.function.name)
                });
            } else if progress.mode == ProgressMode::Full {
                progress.subtool(format!(
                    "__subtool_call__{}",
                    json!({
                        "name": call.function.name,
                        "args": call.function.arguments,
                    })
                ));
            }
            let (output, ok) = match tools
                .call(&call.function.name, &call.function.arguments)
                .await
            {
                Ok(output) => (output, true),
                Err(err) => (format!("tool error: {err}"), false),
            };
            if ok {
                stats.tool_ok += 1;
            } else {
                stats.tool_errors += 1;
            }
            if progress.mode == ProgressMode::Summary {
                progress.report(if is_zh() {
                    format!(
                        "工具 #{steps}：{} ok",
                        readable_tool_name(&call.function.name)
                    )
                } else {
                    format!("tool #{steps}: {} ok", call.function.name)
                });
            } else if progress.mode == ProgressMode::Full {
                progress.subtool(format!(
                    "__subtool_result__{}",
                    json!({
                        "name": call.function.name,
                        "ok": ok,
                        "output": output,
                    })
                ));
            }
            transcript.push(render_internal_tool_result(
                &call.function.name,
                &call.function.arguments,
                ok,
                &output,
            ));
        }
        if !transcript.is_empty() {
            messages.push(ChatMessage::plain(
                "user",
                render_internal_tool_transcript(&transcript, steps, max_tool_steps),
            ));
        }
    }
}

fn render_internal_tool_transcript(results: &[String], steps: usize, max_steps: usize) -> String {
    format!(
        "<subagent_tool_transcript>\n说明：以下是宿主已经执行完成的内部工具调用结果，不是新的用户请求。请基于这些观察继续调查；如证据已经足够，请输出最终报告。\ntool_budget: {steps}/{max_steps}\n{}\n</subagent_tool_transcript>",
        results.join("\n")
    )
}

fn render_internal_tool_result(name: &str, arguments: &str, ok: bool, output: &str) -> String {
    format!(
        "<tool_result name=\"{}\" ok=\"{}\">\narguments_json:\n```json\n{}\n```\noutput:\n```text\n{}\n```\n</tool_result>",
        name,
        ok,
        arguments.trim(),
        clip_inline(output, 6000)
    )
}

fn finalization_prompt() -> &'static str {
    "<tool_budget_reached>工具预算已用尽。不要再请求工具。请只基于上面的用户问题、系统要求和已执行工具结果输出最终调查报告；缺少证据的地方明确写“不确定”或“缺证据”。</tool_budget_reached>"
}

fn strip_report_preamble(content: &str) -> String {
    let trimmed = content.trim();
    for heading in ["## 调查结果", "# 调查结果"] {
        if let Some(index) = trimmed.find(heading) {
            return trimmed[index..].trim().to_string();
        }
    }
    trimmed
        .lines()
        .skip_while(|line| {
            let line = line.trim();
            line.is_empty()
                || line == "---"
                || line.contains("以下是")
                || line.contains("最终报告") && line.len() < 30
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn clip_inline(value: &str, max_chars: usize) -> String {
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if value.chars().count() <= max_chars {
        value
    } else {
        format!(
            "{}...",
            value
                .chars()
                .take(max_chars.saturating_sub(3))
                .collect::<String>()
        )
    }
}

