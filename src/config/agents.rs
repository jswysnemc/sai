use super::agent_presets::{builtin_agent_profiles, resolve_deferred_tools, resolve_enabled_tools};
use super::PromptSectionToggles;
use serde::{Deserialize, Serialize};

pub const DEFAULT_AGENT_ID: &str = "default";
pub const CLI_AGENT_ID: &str = "cli";
pub const GENERAL_AGENT_ID: &str = "general";
pub const EXPLORE_AGENT_ID: &str = "explore";
pub const PLAN_AGENT_ID: &str = "plan";
pub const GATEWAY_AGENT_ID: &str = "gateway";

/// 选择默认 Agent 的运行入口。
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum AgentSurface {
    Web,
    Tui,
    Cli,
    Gateway,
}

/// 仅在单轮运行期间生效的 Agent 能力覆盖。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AgentRuntimeOverride {
    /// 允许使用的工具名称
    pub enabled_tools: Vec<String>,
    /// 白名单是否为最终结果。
    ///
    /// 为真时空列表表示一个工具都不给，且不再补回交互兜底工具；
    /// 为假时沿用旧语义，空列表表示不做收窄。
    pub exclusive: bool,
    /// 需要模型调用 load 后才暴露的工具名称，必须是 `enabled_tools` 的子集
    pub deferred_tools: Vec<String>,
    /// 完整暴露的 skills
    pub skills_full: Vec<String>,
    /// 仅暴露名称的 skills
    pub skills_named: Vec<String>,
}

/// Agent 配置档案。
///
/// 描述一个可复用的 Agent 预设：运行模型、系统提示词、能力集合和注册范围。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentProfile {
    /// Agent 唯一标识
    pub id: String,
    /// Agent 显示名称
    pub name: String,
    /// 主 Agent 选择或委派时展示的用途描述
    #[serde(default)]
    pub description: String,
    /// 系统提示词全文
    #[serde(default)]
    pub system_prompt: String,
    /// 启用的工具，可填写工具名或工具分组名
    #[serde(default)]
    pub enabled_tools: Vec<String>,
    /// 启用工具中需要模型调用 load 后才暴露的部分；其余启用工具会话开始即可见
    #[serde(default)]
    pub deferred_tools: Vec<String>,
    /// 完整启用的 skills：加载名称与描述
    #[serde(default)]
    pub skills_full: Vec<String>,
    /// 半启用的 skills：仅暴露名称
    #[serde(default)]
    pub skills_named: Vec<String>,
    /// 可选供应商 id，空表示沿用当前供应商
    #[serde(default)]
    pub provider_id: String,
    /// 可选模型，空表示沿用供应商当前模型
    #[serde(default)]
    pub model: String,
    /// 可选思考等级，auto 表示沿用当前配置
    #[serde(default = "default_agent_thinking_level")]
    pub thinking_level: String,
    /// 是否向主 Agent 注册为可调用的子 Agent
    #[serde(default)]
    pub register_to_main: bool,
    /// 是否加载全局 / 项目 AGENT.md、AGENTS.md、CLAUDE.md 等指令文件
    #[serde(default = "default_true")]
    pub load_instruction_files: bool,
    /// 工具白名单是否为最终结果。
    ///
    /// false（默认）：空列表表示继承预设或全量工具，与旧配置一致。
    /// true：列表就是最终工具集，空列表即一个工具都不给。
    /// 不复用空列表本身来表达"零工具"，是因为已有配置里存在空列表且
    /// 依赖旧语义，翻转会让那些 Agent 静默失去全部工具。
    #[serde(default)]
    pub tools_exclusive: bool,
    /// 系统提示词各内置分段的开关
    #[serde(default)]
    pub prompt_sections: PromptSectionToggles,
}

/// 旧版可由主 Agent 选择的子 Agent 档案，仅用于配置兼容迁移。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubagentProfile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub system_prompt: String,
    #[serde(default)]
    pub provider_id: String,
    #[serde(default)]
    pub model: String,
    #[serde(default = "default_agent_thinking_level")]
    pub thinking_level: String,
    #[serde(default = "default_true")]
    pub exposed: bool,
}

/// 旧版子智能体运行配置。
///
/// 新配置应改用统一 AgentProfile；这些字段继续支持已有配置。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SubagentConfig {
    /// 子智能体使用的供应商 id，空表示沿用主对话
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub provider_id: String,
    /// 子智能体使用的模型，空表示沿用该供应商默认模型
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub model: String,
    #[serde(default = "default_agent_thinking_level")]
    pub thinking_level: String,
    #[serde(default)]
    pub default_profile: String,
    #[serde(default)]
    pub profiles: Vec<SubagentProfile>,
}

impl Default for AgentProfile {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            description: String::new(),
            system_prompt: String::new(),
            tools_exclusive: false,
            prompt_sections: PromptSectionToggles::default(),
            enabled_tools: Vec::new(),
            deferred_tools: Vec::new(),
            skills_full: Vec::new(),
            skills_named: Vec::new(),
            provider_id: String::new(),
            model: String::new(),
            thinking_level: default_agent_thinking_level(),
            register_to_main: false,
            load_instruction_files: true,
        }
    }
}

impl AgentProfile {
    /// 将旧子 Agent 档案转换为统一 Agent 档案。
    ///
    /// 参数:
    /// - `profile`: 旧子 Agent 档案
    ///
    /// 返回:
    /// - 可用于统一运行时的 Agent 档案
    fn from_legacy_subagent(profile: SubagentProfile) -> Self {
        Self {
            id: profile.id,
            name: profile.name,
            description: profile.description,
            system_prompt: profile.system_prompt,
            enabled_tools: Vec::new(),
            deferred_tools: Vec::new(),
            skills_full: Vec::new(),
            skills_named: Vec::new(),
            provider_id: profile.provider_id,
            model: profile.model,
            thinking_level: profile.thinking_level,
            register_to_main: profile.exposed,
            tools_exclusive: false,
            prompt_sections: PromptSectionToggles::default(),
            load_instruction_files: true,
        }
    }
}

impl crate::config::AppConfig {
    /// 返回包含内置通用、探索和旧配置迁移结果的统一 Agent 列表。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 去重后的 Agent 档案
    pub fn resolved_agent_profiles(&self) -> Vec<AgentProfile> {
        let legacy = &self.subagent.profiles;
        let mut profiles = builtin_agent_profiles()
            .into_iter()
            .map(|builtin| {
                self.agents
                    .iter()
                    .find(|profile| profile.id == builtin.id)
                    .cloned()
                    .or_else(|| {
                        legacy
                            .iter()
                            .find(|profile| profile.id == builtin.id)
                            .cloned()
                            .map(AgentProfile::from_legacy_subagent)
                    })
                    .unwrap_or(builtin)
            })
            .collect::<Vec<_>>();
        for legacy in legacy.iter().cloned() {
            if profiles.iter().any(|profile| profile.id == legacy.id)
                || self.agents.iter().any(|profile| profile.id == legacy.id)
            {
                continue;
            }
            profiles.push(AgentProfile::from_legacy_subagent(legacy));
        }
        profiles.extend(
            self.agents
                .iter()
                .filter(|profile| {
                    !matches!(
                        profile.id.as_str(),
                        CLI_AGENT_ID
                            | GENERAL_AGENT_ID
                            | EXPLORE_AGENT_ID
                            | PLAN_AGENT_ID
                            | GATEWAY_AGENT_ID
                    )
                })
                .cloned(),
        );
        profiles
    }

    /// 解析指定入口默认使用的 Agent 标识。
    ///
    /// 参数:
    /// - `surface`: 当前运行入口
    ///
    /// 返回:
    /// - 配置的 Agent 标识
    pub fn default_agent_for_surface(&self, surface: AgentSurface) -> Option<&str> {
        let value = match surface {
            AgentSurface::Web => self.default_agent.as_deref(),
            AgentSurface::Tui => self.tui_agent.as_deref(),
            AgentSurface::Cli => self.cli_agent.as_deref(),
            AgentSurface::Gateway => self.gateway_agent.as_deref(),
        };
        value.map(str::trim).filter(|value| !value.is_empty())
    }

    /// 返回当前运行期 Agent 需要延迟加载的工具集合。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 需要模型调用 load 后才暴露的工具名；未配置运行期覆盖时返回空切片
    pub(crate) fn agent_deferred_tools(&self) -> &[String] {
        self.agent_runtime
            .as_ref()
            .map(|runtime| runtime.deferred_tools.as_slice())
            .unwrap_or_default()
    }

    /// 判断当前 Agent 是否配置了任何可见 skill。
    ///
    /// skill 提示词只给名称与简介，完整流程一律靠 `load` 读取，
    /// 因此只要有可见 skill 就必须注册加载器，否则模型看得到名字却无从加载。
    /// 未配置运行期覆盖时全部 skill 可见，同样需要加载器。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 是否存在可见 skill
    pub fn has_visible_skills(&self) -> bool {
        self.agent_runtime.as_ref().is_none_or(|runtime| {
            !runtime.skills_full.is_empty() || !runtime.skills_named.is_empty()
        })
    }

    /// 解析主 Agent 可调用的已注册 Agent。
    ///
    /// 参数:
    /// - `requested`: 主 Agent 显式选择的 Agent 标识
    ///
    /// 返回:
    /// - 已注册的 Agent 档案
    pub fn resolve_registered_agent(&self, requested: Option<&str>) -> Option<AgentProfile> {
        let requested = requested
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .or_else(|| {
                (!self.subagent.default_profile.trim().is_empty())
                    .then_some(self.subagent.default_profile.trim())
            })
            .unwrap_or(GENERAL_AGENT_ID);
        self.resolved_agent_profiles()
            .into_iter()
            .find(|profile| profile.register_to_main && profile.id == requested)
    }

    /// 为指定 Agent 写入或清除供应商/模型覆盖。
    ///
    /// 内置与旧版迁移档案尚未落盘时，先物化完整档案再改写，
    /// 避免只含 id 与模型的空档案覆盖工具白名单等内置能力。
    /// 空的供应商标识表示恢复「沿用当前模型」。
    ///
    /// 参数:
    /// - `agent_id`: Agent 标识
    /// - `provider_id`: 供应商标识；空表示沿用当前供应商
    /// - `model`: 模型名称；空表示沿用该供应商当前模型
    ///
    /// 返回:
    /// - 是否改动了配置
    pub fn set_agent_model(&mut self, agent_id: &str, provider_id: &str, model: &str) -> bool {
        let Some(profile) = self
            .resolved_agent_profiles()
            .into_iter()
            .find(|profile| profile.id == agent_id)
        else {
            return false;
        };
        let provider_id = provider_id.trim();
        let model = model.trim();
        if let Some(existing) = self
            .agents
            .iter_mut()
            .find(|existing| existing.id == agent_id)
        {
            let mut changed = false;
            if existing.provider_id != provider_id {
                existing.provider_id = provider_id.to_string();
                changed = true;
            }
            if existing.model != model {
                existing.model = model.to_string();
                changed = true;
            }
            changed
        } else if !provider_id.is_empty()
            || !profile.provider_id.is_empty()
            || !profile.model.is_empty()
        {
            // 未配置档案仅在需要固定或清除既有覆盖时物化，避免无谓膨胀配置
            let mut materialized = profile;
            materialized.provider_id = provider_id.to_string();
            materialized.model = model.to_string();
            self.agents.push(materialized);
            true
        } else {
            false
        }
    }
}

/// 把指定 Agent 档案应用到运行期配置。
///
/// 参数:
/// - `config`: 当前应用配置
/// - `agent_id`: 调用方显式选择的 Agent 标识
/// - `surface`: 当前运行入口
///
/// 返回:
/// - 已应用模型、提示词和能力覆盖的配置
pub fn apply_agent_override(
    mut config: crate::config::AppConfig,
    agent_id: Option<&str>,
    surface: AgentSurface,
) -> anyhow::Result<crate::config::AppConfig> {
    use anyhow::bail;

    // 1. CLI 与 TUI 只使用内置内核，Web 与网关保留多内核能力
    if matches!(surface, AgentSurface::Cli | AgentSurface::Tui) {
        config.agent.engine = super::AgentEngineKind::Native;
    }
    // 2. 显式选择优先，未指定时采用当前入口默认值
    let explicit = agent_id.map(str::trim).filter(|value| !value.is_empty());
    let selected = explicit.map(str::to_string).or_else(|| {
        config
            .default_agent_for_surface(surface)
            .map(str::to_string)
    });
    let Some(agent_id) = selected else {
        return Ok(config);
    };
    // 3. 从统一列表解析内置、旧版迁移或自定义档案
    let profile = config
        .resolved_agent_profiles()
        .into_iter()
        .find(|profile| profile.id == agent_id);
    let Some(profile) = profile else {
        if agent_id == DEFAULT_AGENT_ID {
            return Ok(config);
        }
        bail!("agent not found: {agent_id}");
    };
    // 4. 内置档案的供应商、模型和思考等级为空时沿用全局选择；只有用户明确配置
    //    过档案，或使用自定义档案时，才允许档案固定值覆盖当前选择
    // 分段开关先落到运行配置：后续提示词组装与人设回退都读它
    config.prompt_sections = profile.prompt_sections.clone();
    if !profile.system_prompt.trim().is_empty() {
        config.system_prompt_file = None;
        config.system_prompt = Some(profile.system_prompt.clone());
    } else if !profile.prompt_sections.builtin_persona {
        // 关掉内置人设又没写自己的提示词，就是明确要空白；
        // 这里必须写入空串并清掉文件路径，否则会一路回退到内置人设
        config.system_prompt_file = None;
        config.system_prompt = Some(String::new());
    }
    if profile_pins_model_selection(&profile) {
        if !profile.provider_id.trim().is_empty() {
            config.active_provider = profile.provider_id.clone();
        }
        if let Some(provider) = config
            .providers
            .iter_mut()
            .find(|provider| provider.id == config.active_provider)
        {
            // 5. 自定义档案的模型与思考等级覆盖当前供应商；空值继续沿用全局配置
            if !profile.model.trim().is_empty() {
                provider.default_model = profile.model.clone();
            }
            if !profile.thinking_level.trim().is_empty() && profile.thinking_level != "auto" {
                provider.thinking_level = profile.thinking_level.clone();
            }
        }
    }
    // 6. 工具白名单：空列表表示全量；内置 explore/plan/gateway/code 有默认白名单
    //    延迟集合从白名单中划出需要 load 才暴露的部分，两者一起构成三段状态
    let enabled_tools = resolve_enabled_tools(&profile);
    let deferred_tools = resolve_deferred_tools(&profile, &enabled_tools);
    config.load_instruction_files = profile.load_instruction_files;
    // 独占白名单必须落成覆盖：否则空列表会被当成"没有覆盖"，退回全量工具
    config.agent_runtime = if !profile.tools_exclusive
        && enabled_tools.is_empty()
        && deferred_tools.is_empty()
        && profile.skills_full.is_empty()
        && profile.skills_named.is_empty()
    {
        None
    } else {
        Some(AgentRuntimeOverride {
            enabled_tools,
            exclusive: profile.tools_exclusive,
            deferred_tools,
            skills_full: profile.skills_full,
            skills_named: profile.skills_named,
        })
    };
    Ok(config)
}

/// 判断 Agent 档案是否明确固定了供应商、模型或思考等级。
///
/// 内置档案会写入配置文件，但其中的空选择字段只是“沿用全局选择”的默认值，
/// 不能在每次配置重载时覆盖用户通过 `/model` 或模型选择器选择的结果。
///
/// 参数:
/// - `profile`: 已解析的 Agent 档案
///
/// 返回:
/// - 档案包含明确运行期选择时返回 true
fn profile_pins_model_selection(profile: &AgentProfile) -> bool {
    let Some(builtin) = builtin_agent_profiles()
        .into_iter()
        .find(|builtin| builtin.id == profile.id)
    else {
        // 自定义档案的显式选择应当生效；字段全为空时下面的覆盖逻辑不会改动配置
        return true;
    };
    profile.provider_id != builtin.provider_id
        || profile.model != builtin.model
        || profile.thinking_level != builtin.thinking_level
}

fn default_agent_thinking_level() -> String {
    "auto".to_string()
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证统一 Agent 配置可以覆盖内置探索 Agent 并关闭主 Agent 注册。
    #[test]
    fn unified_agents_override_builtin_registration() {
        let mut config = crate::config::AppConfig::default();
        config.agents.push(AgentProfile {
            id: EXPLORE_AGENT_ID.to_string(),
            name: "项目探索".to_string(),
            description: "只查项目".to_string(),
            register_to_main: false,
            ..AgentProfile::default()
        });

        assert!(config
            .resolved_agent_profiles()
            .iter()
            .any(|profile| profile.id == EXPLORE_AGENT_ID && profile.name == "项目探索"));
        assert!(config
            .resolve_registered_agent(Some(EXPLORE_AGENT_ID))
            .is_none());
    }

    /// 验证 CLI 与 TUI 可以选择不同的默认 Agent。
    #[test]
    fn applies_surface_specific_default_agents() {
        let mut config = crate::config::AppConfig::default();
        config.agents.push(AgentProfile {
            id: "cli-agent".to_string(),
            name: "CLI".to_string(),
            system_prompt: "cli prompt".to_string(),
            ..AgentProfile::default()
        });
        config.agents.push(AgentProfile {
            id: "tui-agent".to_string(),
            name: "TUI".to_string(),
            system_prompt: "tui prompt".to_string(),
            ..AgentProfile::default()
        });
        config.cli_agent = Some("cli-agent".to_string());
        config.tui_agent = Some("tui-agent".to_string());

        let cli = apply_agent_override(config.clone(), None, AgentSurface::Cli).unwrap();
        let tui = apply_agent_override(config, None, AgentSurface::Tui).unwrap();
        assert_eq!(cli.system_prompt.as_deref(), Some("cli prompt"));
        assert_eq!(tui.system_prompt.as_deref(), Some("tui prompt"));
    }

    /// 验证终端入口强制使用内置内核，Web 仍保留外部内核。
    #[test]
    fn terminal_surfaces_force_native_engine_without_changing_web() {
        let mut config = crate::config::AppConfig::default();
        config.agent.engine = crate::config::AgentEngineKind::ClaudeCode;

        let cli = apply_agent_override(config.clone(), None, AgentSurface::Cli).unwrap();
        let tui = apply_agent_override(config.clone(), None, AgentSurface::Tui).unwrap();
        let web = apply_agent_override(config, None, AgentSurface::Web).unwrap();

        assert_eq!(cli.agent.engine, crate::config::AgentEngineKind::Native);
        assert_eq!(tui.agent.engine, crate::config::AgentEngineKind::Native);
        assert_eq!(web.agent.engine, crate::config::AgentEngineKind::ClaudeCode);
    }

    /// 内置代码 Agent 带有工程规范提示词；探索 Agent 为只读。
    #[test]
    fn builtin_agents_include_default_prompts() {
        let [cli, general, explore, plan, _gateway] = builtin_agent_profiles();
        assert!(cli.system_prompt.contains("Sai"));
        assert!(cli.enabled_tools.is_empty());
        assert!(general.system_prompt.contains("核心铁律"));
        assert!(!general.enabled_tools.is_empty());
        assert!(explore.system_prompt.contains("只读"));
        assert!(!explore.enabled_tools.is_empty());
        assert!(plan.system_prompt.contains("Plan"));
        assert!(!plan.enabled_tools.is_empty());
    }

    /// 默认入口：CLI 助手全量；TUI/Web 代码 Agent；网关专用。
    #[test]
    fn default_surfaces_use_cli_and_code_agents() {
        let mut config = crate::config::AppConfig::default();
        crate::config::ensure_surface_agent_defaults(&mut config);
        assert_eq!(config.cli_agent.as_deref(), Some(CLI_AGENT_ID));
        assert_eq!(config.tui_agent.as_deref(), Some(GENERAL_AGENT_ID));
        assert_eq!(config.default_agent.as_deref(), Some(GENERAL_AGENT_ID));
        assert_eq!(config.gateway_agent.as_deref(), Some(GATEWAY_AGENT_ID));
        let cli = apply_agent_override(config.clone(), None, AgentSurface::Cli).unwrap();
        let cli_runtime = cli.agent_runtime.expect("CLI 保留通配符延迟集合");
        assert!(cli_runtime.enabled_tools.is_empty(), "CLI 应继承全量工具");
        assert_eq!(
            cli_runtime.deferred_tools,
            vec![crate::config::DEFERRED_ALL_NON_BASE.to_string()],
            "CLI 非基础工具应按需 load"
        );
        assert!(cli.system_prompt.as_deref().unwrap_or("").contains("Sai"));
        assert!(cli.load_instruction_files);
        let tui = apply_agent_override(config.clone(), None, AgentSurface::Tui).unwrap();
        let runtime = tui.agent_runtime.expect("code agent whitelist");
        assert!(runtime.enabled_tools.iter().any(|t| t == "write_file"));
        assert!(runtime.enabled_tools.iter().any(|t| t == "str_replace"));
        assert!(runtime
            .enabled_tools
            .iter()
            .any(|t| t == "scientific_calculator"));
        assert!(runtime
            .enabled_tools
            .iter()
            .any(|t| t == "online_man_search"));
        assert!(runtime.enabled_tools.iter().any(|t| t == "ssh_list_hosts"));
        assert!(runtime.enabled_tools.iter().any(|t| t == "ssh_run_command"));
        assert!(!runtime.enabled_tools.iter().any(|t| t == "show_meme"));
        assert!(!runtime.enabled_tools.iter().any(|t| t == "calculate"));
        assert!(!runtime.enabled_tools.iter().any(|t| t == "man_page_search"));
        assert!(tui
            .system_prompt
            .as_deref()
            .unwrap_or("")
            .contains("核心铁律"));
        let gateway = apply_agent_override(config, None, AgentSurface::Gateway).unwrap();
        assert!(!gateway.load_instruction_files);
        let gateway_tools = gateway
            .agent_runtime
            .expect("gateway whitelist")
            .enabled_tools;
        assert!(gateway_tools.iter().any(|t| t == "get_weather"));
        assert!(gateway_tools.iter().any(|t| t == "get_exchange_rate"));
        assert!(gateway_tools.iter().any(|t| t == "query_deepseek_status"));
        assert!(gateway_tools.iter().any(|t| t == "online_man_get_page"));
        assert!(!gateway_tools.iter().any(|t| t == "query_weather"));
        assert!(!gateway_tools.iter().any(|t| t == "convert_exchange_rate"));
    }

    /// 验证内置 Agent 不会在配置重载时覆盖用户刚选择的模型。
    #[test]
    fn builtin_agent_keeps_user_selected_model_after_reload() {
        let mut config = crate::config::AppConfig::default();
        crate::config::ensure_surface_agent_defaults(&mut config);
        let provider_id = config.providers[0].id.clone();
        let selected_model = "user-selected-model";
        config
            .set_active_provider_model(&provider_id, selected_model)
            .unwrap();

        let resolved = apply_agent_override(config, None, AgentSurface::Tui).unwrap();

        assert_eq!(resolved.active_provider, provider_id);
        assert_eq!(
            resolved.provider(None).unwrap().default_model,
            selected_model
        );
    }

    /// 验证自定义 Agent 仍然可以固定供应商、模型和思考等级。
    #[test]
    fn custom_agent_can_pin_model_selection() {
        let mut config = crate::config::AppConfig::default();
        let provider_id = config.providers[0].id.clone();
        config.agents.push(AgentProfile {
            id: "pinned-agent".to_string(),
            name: "固定模型".to_string(),
            provider_id: provider_id.clone(),
            model: "pinned-model".to_string(),
            thinking_level: "high".to_string(),
            ..AgentProfile::default()
        });

        let resolved =
            apply_agent_override(config, Some("pinned-agent"), AgentSurface::Web).unwrap();
        let provider = resolved.provider(None).unwrap();
        assert_eq!(resolved.active_provider, provider_id);
        assert_eq!(provider.default_model, "pinned-model");
        assert_eq!(provider.thinking_level, "high");
    }

    #[test]
    fn explore_and_plan_are_readonly_scoped() {
        let config = crate::config::AppConfig::default();
        let explore =
            apply_agent_override(config.clone(), Some(EXPLORE_AGENT_ID), AgentSurface::Web)
                .unwrap();
        let tools = explore.agent_runtime.unwrap().enabled_tools;
        assert!(tools.iter().any(|t| t == "read_file"));
        assert!(!tools.iter().any(|t| t == "edit_file"));
        let plan = apply_agent_override(config, Some(PLAN_AGENT_ID), AgentSurface::Web).unwrap();
        let tools = plan.agent_runtime.unwrap().enabled_tools;
        assert!(tools.iter().any(|t| t == "web_search"));
        assert!(tools.iter().any(|t| t == "online_man_search"));
        assert!(!tools.iter().any(|t| t == "run_command"));
        assert!(!tools.iter().any(|t| t == "fetch_url"));
        assert!(plan.system_prompt.as_deref().unwrap_or("").contains("Plan"));
    }

    /// 白名单原样生效，不再对旧工具名做任何映射。
    #[test]
    fn the_whitelist_is_taken_verbatim() {
        let mut config = crate::config::AppConfig::default();
        config.agents.push(AgentProfile {
            id: "verbatim".to_string(),
            name: "原样".to_string(),
            enabled_tools: vec!["web_fetch".to_string(), "str_replace".to_string()],
            ..AgentProfile::default()
        });

        let resolved = apply_agent_override(config, Some("verbatim"), AgentSurface::Web).unwrap();
        let tools = resolved.agent_runtime.unwrap().enabled_tools;

        assert!(tools.iter().any(|tool| tool == "web_fetch"));
        assert!(tools.iter().any(|tool| tool == "str_replace"));
    }

    /// 验证旧子 Agent 档案会进入统一 Agent 列表并保留暴露状态。
    #[test]
    fn migrates_legacy_subagent_profiles_into_unified_agents() {
        let mut config = crate::config::AppConfig::default();
        config.subagent.profiles = vec![SubagentProfile {
            id: EXPLORE_AGENT_ID.to_string(),
            name: "旧探索".to_string(),
            description: "旧用途".to_string(),
            system_prompt: "旧提示".to_string(),
            provider_id: String::new(),
            model: String::new(),
            thinking_level: "high".to_string(),
            exposed: false,
        }];

        let profile = config
            .resolved_agent_profiles()
            .into_iter()
            .find(|profile| profile.id == EXPLORE_AGENT_ID)
            .unwrap();
        assert_eq!(profile.name, "旧探索");
        assert_eq!(profile.thinking_level, "high");
        assert!(!profile.register_to_main);
    }
}

#[cfg(test)]
mod blank_agent_tests {
    use super::*;
    use crate::config::AppConfig;

    /// 构造一个自定义 Agent 档案并挂进配置。
    ///
    /// 参数:
    /// - `profile`: 待挂载的档案
    ///
    /// 返回:
    /// - 已包含该档案的配置
    fn config_with(profile: AgentProfile) -> AppConfig {
        let mut config = AppConfig::default();
        config.agents = vec![profile];
        config
    }

    /// 构造一个最小可用的自定义档案。
    ///
    /// 参数:
    /// - `id`: 档案标识
    ///
    /// 返回:
    /// - 档案
    fn profile(id: &str) -> AgentProfile {
        AgentProfile {
            id: id.to_string(),
            name: id.to_string(),
            ..AgentProfile::default()
        }
    }

    /// 验证关闭内置人设且未写提示词时得到空白提示词。
    ///
    /// 这是"0 提示词 Agent"的核心：此前空提示词会被当成未设置，
    /// 一路回退到内置人设，配置界面上怎么改都清不掉。
    #[test]
    fn a_blank_agent_produces_an_empty_base_prompt() {
        let dir = tempfile::tempdir().unwrap();
        let paths = crate::paths::SaiPaths::for_tests(dir.path());
        let mut blank = profile("blank");
        blank.prompt_sections = crate::config::PromptSectionToggles::all_disabled();

        let config =
            apply_agent_override(config_with(blank), Some("blank"), AgentSurface::Cli).unwrap();

        assert_eq!(config.base_system_prompt(&paths).unwrap(), "");
        assert_eq!(config.system_prompt(&paths).unwrap(), "");
    }

    /// 验证自己写了提示词时不受内置人设开关影响。
    #[test]
    fn an_explicit_prompt_survives_with_the_builtin_persona_off() {
        let dir = tempfile::tempdir().unwrap();
        let paths = crate::paths::SaiPaths::for_tests(dir.path());
        let mut custom = profile("custom");
        custom.system_prompt = "只回答是或否".to_string();
        custom.prompt_sections = crate::config::PromptSectionToggles::all_disabled();

        let config =
            apply_agent_override(config_with(custom), Some("custom"), AgentSurface::Cli).unwrap();

        assert_eq!(config.base_system_prompt(&paths).unwrap(), "只回答是或否");
    }

    /// 验证独占空白名单产生零工具覆盖。
    ///
    /// 空列表在旧语义下表示"全量"，这里必须落成一个明确的空覆盖，
    /// 否则 Agent 会拿到全部工具。
    #[test]
    fn an_exclusive_empty_whitelist_yields_no_tools() {
        let mut bare = profile("bare");
        bare.tools_exclusive = true;

        let config =
            apply_agent_override(config_with(bare), Some("bare"), AgentSurface::Cli).unwrap();

        let runtime = config.agent_runtime.expect("独占白名单必须落成覆盖");
        assert!(runtime.exclusive);
        assert!(runtime.enabled_tools.is_empty());
    }

    /// 验证独占白名单只保留列出的工具。
    #[test]
    fn an_exclusive_whitelist_keeps_only_what_it_lists() {
        let mut minimal = profile("minimal");
        minimal.tools_exclusive = true;
        minimal.enabled_tools = vec!["run_command".to_string(), "write_file".to_string()];

        let config =
            apply_agent_override(config_with(minimal), Some("minimal"), AgentSurface::Cli).unwrap();

        let runtime = config.agent_runtime.expect("独占白名单必须落成覆盖");
        assert_eq!(runtime.enabled_tools, vec!["run_command", "write_file"]);
    }

    /// 验证非独占的空白名单仍是旧语义。
    ///
    /// 已有配置里存在空列表且依赖"继承全量"，语义翻转会让它们静默失去工具。
    #[test]
    fn a_non_exclusive_empty_whitelist_keeps_the_legacy_meaning() {
        let config = apply_agent_override(
            config_with(profile("legacy")),
            Some("legacy"),
            AgentSurface::Cli,
        )
        .unwrap();

        assert!(config.agent_runtime.is_none());
    }
}
