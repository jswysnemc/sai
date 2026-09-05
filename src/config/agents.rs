#[cfg(test)]
#[path = "agents/model_tests.rs"]
mod model_tests;

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

/// 子任务运行模型的共享默认值及兼容档案。
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
    /// 仅在作为子任务启动时应用的模型覆盖，不修改主对话使用的 Agent 档案
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub model_overrides: std::collections::BTreeMap<String, super::SubagentModelChoice>,
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
    #[cfg(test)]
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
#[path = "agents/blank_agent_tests.rs"]
mod blank_agent_tests;
#[cfg(test)]
#[path = "agents/tests.rs"]
mod tests;
