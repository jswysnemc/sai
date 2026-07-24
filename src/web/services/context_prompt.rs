use crate::agent::{build_base_system_prompt, AgentMode};
use crate::cli::build_tool_registry_with_cached_mcp;
use crate::config::{AgentSurface, AppConfig};
use crate::i18n::Locale;
use crate::llm::{ChatContent, ChatMessage, ToolDefinition};
use crate::memory::MemoryStore;
use crate::paths::SaiPaths;
use crate::state::StateStore;
use crate::tools::{self, ToolRegistry};
use anyhow::Result;
use chrono::Local;
use serde::Serialize;
use std::collections::BTreeSet;
use std::io::IsTerminal;

/// 会话上下文提示词预览（稳定 baseline + 动态系统段 + 工具描述）。
#[derive(Debug, Clone, Serialize)]
pub(crate) struct SessionContextPrompt {
    /// 数据来源：session_baseline 为会话已冻结 baseline；live 为按当前配置即时组装
    pub source: String,
    /// 完整 Markdown 文本
    pub content: String,
    /// 字符数
    pub char_count: usize,
    /// 是否包含 instruction-files 片段
    pub has_instruction_files: bool,
    /// 是否包含技能目录片段
    pub has_skills: bool,
    /// 是否包含工具描述片段
    pub has_tools: bool,
    /// 是否包含关联记忆片段
    pub has_memory: bool,
    /// 是否包含运行时 / 模式等动态系统段
    pub has_dynamic: bool,
    /// 可见工具数量
    pub tool_count: usize,
    /// 实际使用的 Agent 标识（若有）
    pub agent_id: Option<String>,
    /// 各段标题，便于前端标签展示
    pub sections: Vec<String>,
}

/// 读取指定会话的完整上下文提示词预览。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `session_id`: 会话 ID
/// - `workspace_path`: 工作区路径（用于加载项目 AGENT.md）
/// - `agent_id`: 可选 Agent 档案覆盖
/// - `locale`: 界面语言（仅影响预览标题与说明文案，不改变模型侧稳定正文）
///
/// 返回:
/// - 与真实请求尽量对齐的系统段 + 工具描述 Markdown
pub(crate) async fn load_session_context_prompt(
    paths: &SaiPaths,
    session_id: &str,
    workspace_path: &str,
    agent_id: Option<&str>,
    locale: Locale,
) -> Result<SessionContextPrompt> {
    let store = StateStore::for_session(paths, session_id)?;
    let workspace = std::path::PathBuf::from(workspace_path);
    let agent_owned = agent_id.map(str::to_string);
    let paths_owned = paths.clone();

    crate::runtime_cwd::scope(workspace, async move {
        let config = AppConfig::load_or_default(&paths_owned)?;
        let config =
            crate::config::apply_agent_override(config, agent_owned.as_deref(), AgentSurface::Web)?;

        // 1. 稳定 baseline：会话 epoch 优先，否则 live 组装
        let (source, baseline) = match store.context_epoch_baseline()? {
            Some(baseline) if !baseline.trim().is_empty() => {
                ("session_baseline".to_string(), baseline)
            }
            _ => (
                "live".to_string(),
                build_base_system_prompt(&config, &paths_owned, config.tools.enabled, None)?,
            ),
        };

        // 2. 动态系统段：与 chat_base_context_projection / turn 组装对齐
        let dynamic = build_dynamic_sections(&config, &paths_owned, &store, locale)?;

        // 3. 工具定义（请求里作为 tools 参数，不是 system 文本；UI 一并展示）
        let tools_section = build_tools_markdown_section(&config, &paths_owned, &store, locale)?;

        // 4. 按真实请求顺序拼接可读预览（标题与说明按界面语言输出）
        let mut parts = Vec::new();
        let mut sections = Vec::new();
        parts.push(section_block(
            locale.text(
                "1. Stable system prompt (Context Epoch baseline)",
                "1. 稳定系统提示（Context Epoch baseline）",
            ),
            &baseline,
        ));
        sections.push(
            locale
                .text("Stable system prompt", "稳定系统提示")
                .to_string(),
        );

        if !dynamic.mode_reminder.trim().is_empty() {
            parts.push(section_block(
                locale.text("2. Mode reminder", "2. 模式提醒"),
                &dynamic.mode_reminder,
            ));
            sections.push(locale.text("Mode reminder", "模式提醒").to_string());
        }
        if !dynamic.selected_model.trim().is_empty() {
            parts.push(section_block(
                locale.text("3. Selected model label", "3. 当前模型标签"),
                &format!("`{}`", dynamic.selected_model.trim()),
            ));
            sections.push(locale.text("Selected model", "当前模型").to_string());
        }
        if !dynamic.loaded_tools_context.trim().is_empty() {
            parts.push(section_block(
                locale.text("4. Loaded tools context", "4. 已加载工具上下文"),
                &dynamic.loaded_tools_context,
            ));
            sections.push(locale.text("Loaded tools", "已加载工具").to_string());
        }
        if !dynamic.goal_context.trim().is_empty() {
            parts.push(section_block(
                locale.text("5. Goal context", "5. Goal 上下文"),
                &dynamic.goal_context,
            ));
            sections.push("Goal".to_string());
        }
        if !dynamic.compaction_summary.trim().is_empty() {
            parts.push(section_block(
                locale.text("6. Compaction summary / Checkpoint", "6. 压缩摘要 / Checkpoint"),
                &dynamic.compaction_summary,
            ));
            sections.push(locale.text("Compaction summary", "压缩摘要").to_string());
        }
        if !dynamic.runtime_context.trim().is_empty() {
            parts.push(section_block(
                locale.text("7. Runtime context", "7. 运行时上下文"),
                &dynamic.runtime_context,
            ));
            sections.push(locale.text("Runtime", "运行时").to_string());
        }
        if !dynamic.associative_memory.trim().is_empty() {
            parts.push(section_block(
                locale.text(
                    "8. Associative memory (recalled from latest user input)",
                    "8. 关联记忆（按最近用户输入召回）",
                ),
                &dynamic.associative_memory,
            ));
            sections.push(locale.text("Associative memory", "关联记忆").to_string());
        } else if dynamic.memory_enabled {
            parts.push(section_block(
                locale.text("8. Associative memory", "8. 关联记忆"),
                locale.text(
                    "_Memory is enabled; no associative hits for the latest user input (recall changes every turn)._",
                    "_记忆已开启；当前无与最近用户输入匹配的联想结果（联想随每轮输入变化）。_",
                ),
            ));
            sections.push(locale.text("Associative memory", "关联记忆").to_string());
        } else {
            parts.push(section_block(
                locale.text("8. Associative memory", "8. 关联记忆"),
                locale.text("_Memory is disabled._", "_记忆功能已关闭。_"),
            ));
            sections.push(locale.text("Associative memory", "关联记忆").to_string());
        }
        if !dynamic.last_auto_meme.trim().is_empty() {
            parts.push(section_block(
                locale.text("9. Auto meme reminder", "9. 自动表情包提醒"),
                &dynamic.last_auto_meme,
            ));
            sections.push(locale.text("Meme reminder", "表情包提醒").to_string());
        }
        if !tools_section.markdown.trim().is_empty() {
            parts.push(tools_section.markdown.trim().to_string());
            sections.push(format!(
                "{} ({})",
                locale.text("Tool definitions", "工具定义"),
                tools_section.tool_count
            ));
        }

        let content = parts
            .into_iter()
            .filter(|part| !part.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n\n---\n\n");

        Ok(summarize_prompt(
            &source,
            content,
            tools_section.tool_count,
            !dynamic.associative_memory.trim().is_empty(),
            dynamic.has_dynamic_system,
            agent_owned,
            sections,
        ))
    })
    .await
}

/// 动态系统段集合。
struct DynamicSections {
    mode_reminder: String,
    selected_model: String,
    loaded_tools_context: String,
    goal_context: String,
    compaction_summary: String,
    runtime_context: String,
    associative_memory: String,
    last_auto_meme: String,
    memory_enabled: bool,
    has_dynamic_system: bool,
}

/// 构造与 Agent 请求对齐的动态系统段。
///
/// 参数:
/// - `config`: 已应用 Agent 覆盖的配置
/// - `paths`: Sai 路径
/// - `store`: 会话状态
/// - `locale`: 界面语言
///
/// 返回:
/// - 动态段集合
fn build_dynamic_sections(
    config: &AppConfig,
    paths: &SaiPaths,
    store: &StateStore,
    locale: Locale,
) -> Result<DynamicSections> {
    // 1. Web 默认 YOLO 模式提醒（与真实 Web run 默认一致；用户切换模式后仍以当前会话最常见路径展示）
    let mode_reminder = AgentMode::Yolo.reminder().to_string();

    // 2. 当前模型标签
    let selected_model = selected_model_label(config)?.unwrap_or_default();

    // 3. 渐进加载已载入工具提示
    let loaded = store.load_loaded_tools().unwrap_or_default();
    let progressive = config.tools.progressive_loading_enabled;
    let loaded_tools_context = if progressive && !loaded.is_empty() {
        format!(
            "<loaded_tools>\nThe following tools are already loaded in this conversation. Do not call load for them again; call the loaded tool directly. If one of these tools returns an error, treat it as an execution or workflow error, not as a loading error.\nLoaded tools: {}\n</loaded_tools>",
            loaded.join(", ")
        )
    } else {
        String::new()
    };

    // 4. Goal 上下文
    let goal_context = store
        .goal()?
        .map(|goal| crate::goal::system_context(&goal))
        .unwrap_or_default();

    // 5. 压缩摘要 / checkpoint
    let projected_history = store.project_history(None)?;
    let compaction_summary = projected_history
        .checkpoint_context
        .or(store.compaction_summary_context()?)
        .unwrap_or_default();

    // 6. 运行时上下文（时间 / 工作目录 / 终端环境）
    let runtime_context = runtime_context_message(locale);

    // 7. 关联记忆：用最近一条用户消息作为查询（真实请求按当前输入召回）
    let memory = config.memory_config();
    let memory_enabled = memory.enabled && memory.association_enabled;
    let associative_memory = if memory_enabled {
        let query = latest_user_text(&projected_history.messages);
        if query.trim().is_empty() {
            String::new()
        } else {
            let memory = MemoryStore::new(config, paths);
            memory
                .association(&query)?
                .map(|association| memory.format_association(&association))
                .unwrap_or_default()
        }
    } else {
        String::new()
    };

    // 8. 最近自动表情包提醒
    let last_auto_meme =
        crate::tools::memes::last_auto_meme_reminder(config, paths)?.unwrap_or_default();

    let has_dynamic_system = [
        mode_reminder.as_str(),
        selected_model.as_str(),
        loaded_tools_context.as_str(),
        goal_context.as_str(),
        compaction_summary.as_str(),
        runtime_context.as_str(),
        associative_memory.as_str(),
        last_auto_meme.as_str(),
    ]
    .iter()
    .any(|part| !part.trim().is_empty());

    Ok(DynamicSections {
        mode_reminder,
        selected_model,
        loaded_tools_context,
        goal_context,
        compaction_summary,
        runtime_context,
        associative_memory,
        last_auto_meme,
        memory_enabled,
        has_dynamic_system,
    })
}

/// 构造当前配置的 provider/model 标签。
///
/// 参数:
/// - `config`: 应用配置
///
/// 返回:
/// - 当前 provider/model 标签
fn selected_model_label(config: &AppConfig) -> Result<Option<String>> {
    let provider = config.provider(None)?;
    let model = provider.default_model.trim();
    if model.is_empty() {
        return Ok(None);
    }
    let provider_name = provider.display_name.trim();
    let provider_label = if provider_name.is_empty() {
        provider.id.trim()
    } else {
        provider_name
    };
    if provider_label.is_empty() {
        Ok(Some(model.to_string()))
    } else {
        Ok(Some(format!("{provider_label}/{model}")))
    }
}

/// 构造运行时上下文消息（对齐 agent::message_context，文案随界面语言切换）。
///
/// 参数:
/// - `locale`: 界面语言
///
/// 返回:
/// - 运行时 system-reminder 文本
fn runtime_context_message(locale: Locale) -> String {
    let cwd = crate::runtime_cwd::current_dir()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let runtime = terminal_runtime_context(locale);
    let now = if matches!(locale, Locale::Zh) {
        Local::now().format("%Y年%m月%d日 %A %H:%M").to_string()
    } else {
        Local::now().format("%Y-%m-%d %A %H:%M").to_string()
    };
    match locale {
        Locale::Zh => format!(
            "<system-reminder>\n当前系统时间：{now}。用户询问当前时间时，优先使用这里的时间，不需要调用命令查询。\n当前工作目录：{cwd}。涉及相对路径、当前项目、文件操作时优先以此为准。\n{runtime}\n</system-reminder>"
        ),
        Locale::En => format!(
            "<system-reminder>\nCurrent system time: {now}. When the user asks about the current time, prefer this value and do not run a command to query it.\nCurrent working directory: {cwd}. Prefer this path for relative paths, the current project, and file operations.\n{runtime}\n</system-reminder>"
        ),
    }
}

/// 构造终端运行环境描述。
///
/// 参数:
/// - `locale`: 界面语言
///
/// 返回:
/// - 终端环境说明
fn terminal_runtime_context(locale: Locale) -> String {
    let stdin_tty = std::io::stdin().is_terminal();
    let stdout_tty = std::io::stdout().is_terminal();
    let stderr_tty = std::io::stderr().is_terminal();
    let environment = if stdin_tty || stdout_tty || stderr_tty {
        locale.text("terminal session", "终端会话")
    } else {
        locale.text(
            "non-interactive or piped environment",
            "非交互或管道环境",
        )
    };
    let shell = std::env::var("SHELL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    let mut terminal_parts = Vec::new();
    for key in ["TERM_PROGRAM", "TERM", "COLORTERM"] {
        if let Ok(value) = std::env::var(key) {
            if !value.trim().is_empty() {
                terminal_parts.push(format!("{key}={value}"));
            }
        }
    }
    let terminal = if terminal_parts.is_empty() {
        "unknown".to_string()
    } else {
        terminal_parts.join(", ")
    };
    match locale {
        Locale::Zh => format!(
            "当前运行环境：{environment}。当前 shell：{shell}。当前终端标识：{terminal}。"
        ),
        Locale::En => format!(
            "Current runtime environment: {environment}. Current shell: {shell}. Terminal identifiers: {terminal}."
        ),
    }
}

/// 提取历史中最近一条用户文本。
///
/// 参数:
/// - `messages`: 投影历史消息
///
/// 返回:
/// - 最近用户输入文本
fn latest_user_text(messages: &[ChatMessage]) -> String {
    messages
        .iter()
        .rev()
        .find(|message| message.role == "user")
        .map(|message| chat_content_text(message.content.as_ref()))
        .unwrap_or_default()
}

/// 提取消息文本内容。
///
/// 参数:
/// - `content`: 消息内容
///
/// 返回:
/// - 纯文本
fn chat_content_text(content: Option<&ChatContent>) -> String {
    match content {
        Some(ChatContent::Text(text)) => text.clone(),
        Some(ChatContent::Parts(parts)) => parts
            .iter()
            .filter_map(|part| match part {
                crate::llm::ChatContentPart::Text { text } => Some(text.as_str()),
                crate::llm::ChatContentPart::ImageUrl { .. } => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        None => String::new(),
    }
}

/// 包装带标题的 Markdown 段。
///
/// 参数:
/// - `title`: 段标题
/// - `body`: 段正文
///
/// 返回:
/// - Markdown 段
fn section_block(title: &str, body: &str) -> String {
    format!("## {title}\n\n{}", body.trim())
}

/// 工具 Markdown 片段。
struct ToolsMarkdownSection {
    markdown: String,
    tool_count: usize,
}

/// 构造当前会话可见工具的 Markdown 描述。
///
/// 参数:
/// - `config`: 已应用 Agent 覆盖的配置
/// - `paths`: Sai 路径
/// - `store`: 会话状态（用于已加载工具）
/// - `locale`: 界面语言
///
/// 返回:
/// - 工具描述片段与数量
fn build_tools_markdown_section(
    config: &AppConfig,
    paths: &SaiPaths,
    store: &StateStore,
    locale: Locale,
) -> Result<ToolsMarkdownSection> {
    if !config.tools.enabled {
        return Ok(ToolsMarkdownSection {
            markdown: String::new(),
            tool_count: 0,
        });
    }

    // 1. 构建缓存 MCP 的注册表，并应用 Web Agent 工具过滤
    let mut registry = build_tool_registry_with_cached_mcp(config, paths, AgentMode::Yolo)?;
    apply_web_agent_tool_filter(config, &mut registry)?;

    // 2. 渐进加载时仅展示初始工具 + 已加载工具
    let loaded = store.load_loaded_tools().unwrap_or_default();
    let progressive = config.tools.progressive_loading_enabled;
    let visible_names = visible_tool_names(&registry, progressive, &loaded);
    let mut definitions = registry.definitions_for_names(&visible_names);
    definitions.sort_by(|left, right| left.function.name.cmp(&right.function.name));
    if definitions.is_empty() {
        return Ok(ToolsMarkdownSection {
            markdown: String::new(),
            tool_count: 0,
        });
    }

    // 3. 渲染为可读 Markdown
    let mut out = String::new();
    out.push_str(&format!(
        "## {}\n\n",
        locale.text(
            "10. Tool definitions (request tools parameter)",
            "10. 工具定义（请求 tools 参数）",
        )
    ));
    out.push_str(&format!(
        "{}\n\n",
        locale
            .text(
                "This session exposes **{count}** tool definitions (name, description, and parameter schema) to the model.",
                "当前会话对模型暴露 **{count}** 个工具定义（名称、描述与参数 schema）。",
            )
            .replace("{count}", &definitions.len().to_string())
    ));
    if progressive {
        out.push_str(locale.text(
            "Progressive loading is enabled: the list below shows currently visible tools (initial tools plus tools already loaded in this session).\n\n",
            "渐进加载已开启：下列为当前可见工具（初始工具 + 本会话已 load 的工具）。\n\n",
        ));
    }
    for definition in &definitions {
        out.push_str(&format_tool_definition_markdown(definition));
        out.push('\n');
    }

    Ok(ToolsMarkdownSection {
        tool_count: definitions.len(),
        markdown: out,
    })
}

/// 将单个工具定义格式化为 Markdown。
///
/// 参数:
/// - `definition`: 工具定义
///
/// 返回:
/// - Markdown 片段
fn format_tool_definition_markdown(definition: &ToolDefinition) -> String {
    let name = &definition.function.name;
    let description = definition.function.description.trim();
    let parameters = serde_json::to_string_pretty(&definition.function.parameters)
        .unwrap_or_else(|_| definition.function.parameters.to_string());
    format!("### `{name}`\n\n{description}\n\n```json\n{parameters}\n```\n")
}

/// 汇总提示词元信息。
///
/// 参数:
/// - `source`: 数据来源标记
/// - `content`: 提示词正文
/// - `tool_count`: 工具数量
/// - `has_memory`: 是否含关联记忆正文
/// - `has_dynamic`: 是否含动态系统段
/// - `agent_id`: 可选 Agent 标识
/// - `sections`: 段标题列表
///
/// 返回:
/// - 带元信息的预览结构
fn summarize_prompt(
    source: &str,
    content: String,
    tool_count: usize,
    has_memory: bool,
    has_dynamic: bool,
    agent_id: Option<String>,
    sections: Vec<String>,
) -> SessionContextPrompt {
    let has_instruction_files = content.contains("<instruction-files>")
        || content.contains("## 指令文件")
        || content.contains("## Instruction files")
        || content.contains("instruction-file");
    let has_skills = content.contains("<available-skills>")
        || content.contains("技能目录")
        || content.contains("Available skills");
    let has_tools = tool_count > 0
        || content.contains("工具定义")
        || content.contains("Tool definitions");
    let char_count = content.chars().count();
    SessionContextPrompt {
        source: source.to_string(),
        content,
        char_count,
        has_instruction_files,
        has_skills,
        has_tools,
        has_memory,
        has_dynamic,
        tool_count,
        agent_id,
        sections,
    }
}

/// 应用 Web Agent 工具白名单过滤。
///
/// 参数:
/// - `config`: 应用配置
/// - `registry`: 待过滤注册表
///
/// 返回:
/// - 是否成功
fn apply_web_agent_tool_filter(config: &AppConfig, registry: &mut ToolRegistry) -> Result<()> {
    let Some(runtime) = config.agent_runtime.as_ref() else {
        return Ok(());
    };
    let allowed = runtime
        .enabled_tools
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let mut filtered = registry.clone_filtered(&allowed);
    for name in ["subagent", "todo", "ask_question"] {
        if registry.contains(name) {
            let _ = filtered.register_from(registry, name);
        }
    }
    *registry = filtered;
    Ok(())
}

/// 计算当前可见工具名集合。
///
/// 参数:
/// - `registry`: 工具注册表
/// - `progressive`: 是否渐进加载
/// - `loaded`: 已加载工具
///
/// 返回:
/// - 可见工具名集合
fn visible_tool_names(
    registry: &ToolRegistry,
    progressive: bool,
    loaded: &[String],
) -> BTreeSet<String> {
    let loaded_set: BTreeSet<String> = loaded.iter().cloned().collect();
    registry
        .tool_infos()
        .into_iter()
        .filter(|info| {
            !progressive || tools::is_initial_tool(&info.name) || loaded_set.contains(&info.name)
        })
        .map(|info| info.name)
        .collect()
}
