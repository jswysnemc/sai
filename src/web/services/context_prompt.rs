use super::context_prompt_section::{
    mode_preview, section, split_tagged_section, ContextPromptSection,
};
use super::context_runtime::project_context_runtime;
use crate::agent::{build_base_system_prompt, AgentMode};
use crate::cli::build_tool_registry_with_cached_mcp;
use crate::config::AppConfig;
use crate::i18n::Locale;
use crate::llm::ToolDefinition;
use crate::paths::SaiPaths;
use crate::state::StateStore;
use crate::tools::{self, ToolRegistry};
use anyhow::Result;
use serde::Serialize;

/// 会话上下文提示词预览（稳定 baseline + 动态系统段 + 工具描述）。
#[derive(Debug, Clone, Serialize)]
pub(crate) struct SessionContextPrompt {
    /// 数据来源：session_baseline 为会话已冻结 baseline；live 为按当前配置即时组装
    pub source: String,
    /// 完整 Markdown 文本
    pub content: String,
    /// 字符数
    pub char_count: usize,
    /// 预估 token 数，与实际送入模型的口径一致（tiktoken o200k_base）
    pub token_count: usize,
    /// 是否包含 instruction-files 片段
    pub has_instruction_files: bool,
    /// 是否包含技能目录片段
    pub has_skills: bool,
    /// 是否包含工具描述片段
    pub has_tools: bool,
    /// 是否包含记忆索引片段
    pub has_memory: bool,
    /// 是否包含运行时 / 模式等动态系统段
    pub has_dynamic: bool,
    /// 可见工具数量
    pub tool_count: usize,
    /// 实际使用的 Agent 标识（若有）
    pub agent_id: Option<String>,
    /// 带稳定 ID 的预览分区，供前端展示与导航
    pub sections: Vec<ContextPromptSection>,
}

/// 读取指定会话的完整上下文提示词预览。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `session_id`: 会话 ID
/// - `workspace_path`: 工作区路径（用于加载项目 AGENT.md）
/// - `agent_id`: 可选 Agent 档案覆盖
/// - `provider_id`: 可选供应商覆盖
/// - `model`: 可选模型覆盖
/// - `mode`: 当前运行模式
/// - `locale`: 界面语言（仅影响预览标题与说明文案，不改变模型侧稳定正文）
///
/// 返回:
/// - 与真实请求尽量对齐的系统段 + 工具描述 Markdown
pub(crate) async fn load_session_context_prompt(
    paths: &SaiPaths,
    session_id: &str,
    workspace_path: &str,
    agent_id: Option<&str>,
    provider_id: Option<&str>,
    model: Option<&str>,
    mode: AgentMode,
    locale: Locale,
) -> Result<SessionContextPrompt> {
    let store = StateStore::for_session(paths, session_id)?;
    let workspace = std::path::PathBuf::from(workspace_path);
    let workspace_path_owned = workspace_path.to_string();
    let agent_owned = agent_id.map(str::to_string);
    let paths_owned = paths.clone();

    crate::runtime_cwd::scope(workspace, async move {
        let config = crate::web::runs::model_override::resolve_run_config(
            &paths_owned,
            agent_owned.as_deref(),
            provider_id,
            model,
            None,
        )?
        .unwrap_or(AppConfig::load_or_default(&paths_owned)?);

        // 1. 稳定系统提示不随模式变化；模式通过后续 user 状态标签载入
        let tools_enabled = config.tools.enabled && config.active_model_tools_enabled()?;
        let live_baseline =
            build_base_system_prompt(&config, &paths_owned, tools_enabled, None)?;
        let (source, baseline) = match store.context_epoch_baseline()? {
            Some(baseline) if baseline == live_baseline => {
                ("session_baseline".to_string(), baseline)
            }
            _ => ("live".to_string(), live_baseline),
        };

        // 2. 动态系统段：与 chat_base_context_projection / turn 组装对齐
        let dynamic = project_context_runtime(
            &config,
            &paths_owned,
            &store,
            &workspace_path_owned,
            mode,
        )?;

        // 3. 工具定义（请求里作为 tools 参数，不是 system 文本；UI 一并展示）
        let tools_section =
            build_tools_markdown_section(&config, &paths_owned, &store, mode, locale)?;

        // 4. 按真实请求顺序构造带稳定标识的可读分区
        let mut sections = Vec::new();
        let (baseline_without_skills, skills_prompt) =
            split_tagged_section(&baseline, "available-skills");
        let (stable_prompt, instruction_from_baseline) =
            split_tagged_section(&baseline_without_skills, "instruction-files");
        sections.push(section(
            "baseline",
            locale.text("Session baseline", "会话 baseline"),
            locale.text(
                "1. Stable system prompt (Context Epoch baseline)",
                "1. 稳定系统提示（Context Epoch baseline）",
            ),
            &stable_prompt,
        ));
        let instruction_files = if !instruction_from_baseline.trim().is_empty() {
            instruction_from_baseline
        } else {
            dynamic.instruction_files.clone()
        };
        if !instruction_files.trim().is_empty() {
            sections.push(section(
                "instructions",
                "AGENT.md",
                locale.text("2. Instruction files", "2. 指令文件"),
                &instruction_files,
            ));
        }
        if !skills_prompt.trim().is_empty() {
            sections.push(section(
                "skills",
                locale.text("Skills catalog", "技能目录"),
                locale.text("3. Skills catalog", "3. 技能目录"),
                &skills_prompt,
            ));
        }

        if !dynamic.goal_context.trim().is_empty() {
            sections.push(section(
                "goal",
                "Goal",
                locale.text("4. Goal context", "4. Goal 上下文"),
                &dynamic.goal_context,
            ));
        }
        if !dynamic.compaction_summary.trim().is_empty() {
            sections.push(section(
                "checkpoint",
                locale.text("Compaction summary", "压缩摘要"),
                locale.text("5. Compaction summary / Checkpoint", "5. 压缩摘要 / Checkpoint"),
                &dynamic.compaction_summary,
            ));
        }
        if !dynamic.runtime_context.trim().is_empty() {
            sections.push(section(
                "runtime",
                locale.text("Runtime", "运行时"),
                locale.text("6. Runtime context", "6. 运行时上下文"),
                &dynamic.runtime_context,
            ));
        }
        if config.prompt_sections.mode_reminder {
            sections.push(section(
                "mode",
                locale.text("Mode instructions", "模式说明"),
                locale.text("7. Current mode instructions", "7. 当前模式说明"),
                &mode_preview(mode, locale),
            ));
        }
        if !dynamic.memory_index.trim().is_empty() {
            sections.push(section(
                "memory",
                locale.text("Memory index", "记忆索引"),
                locale.text(
                    "8. Memory index (injected in full; bodies read on demand)",
                    "8. 记忆索引（全量注入，正文按需读取）",
                ),
                &dynamic.memory_index,
            ));
        } else if dynamic.memory_enabled {
            sections.push(section(
                "memory",
                locale.text("Memory index", "记忆索引"),
                locale.text("8. Memory index", "8. 记忆索引"),
                locale.text(
                    "_Memory is enabled; nothing has been recorded yet. The index appears here once the first entry is written._",
                    "_记忆已开启，但还没有写过任何条目。写下第一条后，索引会出现在这里。_",
                ),
            ));
        } else {
            sections.push(section(
                "memory",
                locale.text("Memory index", "记忆索引"),
                locale.text("8. Memory index", "8. 记忆索引"),
                locale.text("_Memory is disabled._", "_记忆功能已关闭。_"),
            ));
        }
        if !dynamic.last_auto_meme.trim().is_empty() {
            sections.push(section(
                "meme",
                locale.text("Meme reminder", "表情包提醒"),
                locale.text("9. Auto meme reminder", "9. 自动表情包提醒"),
                &dynamic.last_auto_meme,
            ));
        }
        if !tools_section.markdown.trim().is_empty() {
            sections.push(section(
                "tools",
                format!(
                    "{} ({})",
                    locale.text("Tool definitions", "工具定义"),
                    tools_section.tool_count
                ),
                locale.text(
                    "10. Tool definitions (request tools parameter)",
                    "10. 工具定义（请求 tools 参数）",
                ),
                &tools_section.markdown,
            ));
        }

        let content = sections
            .iter()
            .map(|value| value.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n---\n\n");
        let token_count = super::context_breakdown::estimate_context_breakdown_with_runtime(
            &config,
            &paths_owned,
            &store,
            mode,
            &dynamic,
        )?
        .total();

        Ok(summarize_prompt(
            &source,
            content,
            token_count,
            tools_section.tool_count,
            !dynamic.memory_index.trim().is_empty(),
            dynamic.has_dynamic(),
            agent_owned,
            sections,
        ))
    })
    .await
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
/// - `mode`: 当前运行模式
/// - `locale`: 界面语言
///
/// 返回:
/// - 工具描述片段与数量
fn build_tools_markdown_section(
    config: &AppConfig,
    paths: &SaiPaths,
    store: &StateStore,
    mode: AgentMode,
    locale: Locale,
) -> Result<ToolsMarkdownSection> {
    if !config.tools.enabled {
        return Ok(ToolsMarkdownSection {
            markdown: String::new(),
            tool_count: 0,
        });
    }

    // 1. 构建缓存 MCP 的注册表（与 Web 提交路径一致，避免拉起网络发现）
    let mut registry = build_tool_registry_with_cached_mcp(config, paths, mode)?;

    // 2. 注册交互式会话工具：todo / subagent / ask_question
    //    真实 Web run 在 build_submission_tool_registry 中完成，预览原先漏掉
    tools::register_interactive_tools(
        &mut registry,
        config,
        paths,
        store.state_dir().display().to_string(),
        store.session_id().to_string(),
    );

    // 3. 应用 Web Agent 工具白名单（并强制保留 subagent/todo/ask_question）
    apply_web_agent_tool_filter(config, &mut registry)?;

    // 4. 与 Agent::new 对齐：过滤后再挂 goal 工具与渐进 load
    //    create_goal / get_goal / update_goal / load 不依赖 enabled_tools 白名单
    crate::goal::register_tools_for_config(&mut registry, store.goal_file(), config)?;
    let deferred = config.agent_deferred_tools();
    let progressive = !deferred.is_empty();
    if progressive {
        tools::register_progressive_loader(&mut registry, deferred);
    }

    // 5. 渐进模式只展示固定网关，已加载状态不会改变供应商 tools 数组
    let visible_names = tools::progressive::visible_tool_names(&registry, deferred);
    let definitions = registry.definitions_for_names(&visible_names);
    if definitions.is_empty() {
        return Ok(ToolsMarkdownSection {
            markdown: String::new(),
            tool_count: 0,
        });
    }

    // 6. 渲染为可读 Markdown
    let mut out = String::new();
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
            "Progressive loading is enabled: the provider-visible tool list is fixed to load and invoke_tool. The load result returns the target schema; invoke_tool dispatches the validated real call.\n\n",
            "渐进加载已开启：供应商可见工具固定为 load 与 invoke_tool。load 结果返回目标 Schema，invoke_tool 分派通过校验的真实调用。\n\n",
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
/// - `token_count`: 与请求分项一致的已加载上下文 token 数
/// - `tool_count`: 工具数量
/// - `has_memory`: 是否含记忆索引正文
/// - `has_dynamic`: 是否含动态系统段
/// - `agent_id`: 可选 Agent 标识
/// - `sections`: 带稳定 ID 的预览分区
///
/// 返回:
/// - 带元信息的预览结构
fn summarize_prompt(
    source: &str,
    content: String,
    token_count: usize,
    tool_count: usize,
    has_memory: bool,
    has_dynamic: bool,
    agent_id: Option<String>,
    sections: Vec<ContextPromptSection>,
) -> SessionContextPrompt {
    let has_instruction_files = content.contains("<instruction-files>")
        || content.contains("## 指令文件")
        || content.contains("## Instruction files")
        || content.contains("instruction-file");
    let has_skills = content.contains("<available-skills>")
        || content.contains("技能目录")
        || content.contains("Available skills");
    let has_tools =
        tool_count > 0 || content.contains("工具定义") || content.contains("Tool definitions");
    let char_count = content.chars().count();
    SessionContextPrompt {
        source: source.to_string(),
        content,
        char_count,
        token_count,
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
    if !runtime.exclusive {
        for name in ["subagent", "todo", "ask_question"] {
            if registry.contains(name) {
                filtered.register_from(registry, name)?;
            }
        }
    }
    *registry = filtered;
    Ok(())
}
