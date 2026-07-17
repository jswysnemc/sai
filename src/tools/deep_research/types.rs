use super::{readable_tool_name, tool_output_for_context, ToolProgress, ToolRegistry, ToolSpec};
use crate::config::{AppConfig, DeepResearchPluginConfig};
use crate::i18n::{is_zh, text as t};
use crate::llm::{
    ChatMessage, ChatResult, ChatStreamChunk, ChatStreamKind, OpenAiCompatibleClient, Usage,
};
use crate::paths::SaiPaths;
use anyhow::{bail, Result};
use chrono::Local;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const THINKER_SYSTEM_PROMPT: &str = r#"你是 Sai 深度研究系统中的“沉思者”。
你的任务是理解用户命题，主动调用可用工具查证，形成可发送给用户的 Markdown 草稿。

工作原则：
1. 优先基于题面和本地资料；需要时使用 web_search 和 web_fetch 联网查证。
2. 关键事实、技术判断、推荐理由和核心观点应有来源或依据。
3. 需要引用资料时，先调用 register_deep_research_reference 注册参考资料，再在正文中使用返回的 [R数字]/[K数字]/[W数字]。
4. 第一轮必须调用 register_deep_research_topic_title 注册 4-40 字短标题。
5. 不编造来源；资料冲突时说明冲突和取舍；无法查证的点写入“不确定点”。
6. 输出可直接发送给用户的 Markdown 正文，不输出内部 JSON，不输出“参考资料”章节。
7. 不使用 emoji 或装饰性图标。
"#;

const REVIEWER_SYSTEM_PROMPT: &str = r#"你是 Sai 深度研究系统中的“审视者”。
你只审查沉思者草稿，不替用户回答。请严格输出 JSON。

审查重点：
1. 是否覆盖用户问题的关键对象、维度、限制和输出要求。
2. 关键事实和观点是否有已注册 R/K/W 引用支撑。
3. 是否存在严重逻辑错误、前后矛盾、结论超出证据。
4. 是否存在影响结论的数据缺口，却没有说明查证失败或列入不确定点。

输出格式：
{
  "accepted": true/false,
  "challenge": "主要质疑或通过理由",
  "revision_instructions": ["需要修正的事项"]
}
"#;

#[derive(Clone)]
struct DeepResearchContext {
    config: AppConfig,
    paths: SaiPaths,
    tools: ToolRegistry,
}

#[derive(Clone)]
struct ResearchProgress {
    progress: ToolProgress,
    mode: ResearchProgressMode,
    enabled: bool,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ResearchProgressMode {
    Hidden,
    Summary,
    Full,
}

impl ResearchProgress {
    fn new(config: &AppConfig, progress: ToolProgress) -> Self {
        let mode = match config
            .display
            .tool_calls
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "hidden" => ResearchProgressMode::Hidden,
            "full" => ResearchProgressMode::Full,
            _ => ResearchProgressMode::Summary,
        };
        Self {
            progress,
            mode,
            enabled: config.plugins.deep_research.show_progress,
        }
    }

    fn phase(&self, message: impl Into<String>) {
        if self.enabled && self.mode != ResearchProgressMode::Hidden {
            self.progress.report(message.into());
        }
    }

    fn tool(&self, message: impl Into<String>) {
        if self.enabled && self.mode != ResearchProgressMode::Hidden {
            self.progress.report(message.into());
        }
    }

    fn subtool(&self, message: impl Into<String>) {
        if self.enabled && self.mode == ResearchProgressMode::Full {
            self.progress.report(message.into());
        }
    }

    fn reasoning(&self, text: &str) {
        if self.enabled && self.mode != ResearchProgressMode::Hidden {
            self.progress
                .report(format!("__subagent_reasoning__{}", text));
        }
    }

    fn subtool_text(&self, message: impl Into<String>) {
        if self.enabled && self.mode == ResearchProgressMode::Summary {
            self.progress.report(message.into());
        }
    }
}

#[derive(Default)]
struct ResearchState {
    topic_title: String,
    references: Vec<Reference>,
    counters: ReferenceCounters,
    stats: ResearchStats,
}

#[derive(Default)]
struct ResearchStats {
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

impl ResearchStats {
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
}

#[derive(Default)]
struct ReferenceCounters {
    record: usize,
    knowledge: usize,
    web: usize,
}

#[derive(Clone)]
struct Reference {
    marker: String,
    kind: String,
    title: String,
    url: String,
    path: String,
    snippet: String,
}

