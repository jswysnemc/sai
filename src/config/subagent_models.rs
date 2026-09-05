use super::{AgentProfile, AppConfig};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

/// 子任务的模型和思考选择，空模型与 auto 思考分别表示继承。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubagentModelChoice {
    #[serde(default)]
    pub provider_id: String,
    #[serde(default)]
    pub model: String,
    #[serde(default = "auto_thinking")]
    pub thinking_level: String,
}

impl Default for SubagentModelChoice {
    /// 创建继承上层模型与思考的选择，无参数，返回默认值。
    fn default() -> Self {
        Self {
            provider_id: String::new(),
            model: String::new(),
            thinking_level: auto_thinking(),
        }
    }
}

/// 可单独配置的子任务类型及其已保存的运行参数。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct SubagentModelProfile {
    pub id: String,
    pub name: String,
    #[serde(flatten)]
    pub selection: SubagentModelChoice,
}

/// Web 与 TUI 共用的子任务模型设置视图。
#[derive(Debug, Clone, Serialize)]
pub(crate) struct SubagentModelSettings {
    pub defaults: SubagentModelChoice,
    pub profiles: Vec<SubagentModelProfile>,
}

impl AppConfig {
    /// 清理已删除模型的子任务引用；参数为供应商和可选模型，返回值为空。
    pub(super) fn clear_subagent_model_references(
        &mut self,
        provider_id: &str,
        model: Option<&str>,
    ) {
        if self.subagent.provider_id == provider_id
            && model.is_none_or(|model| self.subagent.model == model)
        {
            self.subagent.provider_id.clear();
            self.subagent.model.clear();
        }
        for choice in self.subagent.model_overrides.values_mut() {
            if choice.provider_id == provider_id && model.is_none_or(|model| choice.model == model)
            {
                choice.provider_id.clear();
                choice.model.clear();
            }
        }
    }

    /// 【配置】【子任务模型】读取共享默认值和已注册的子任务覆盖。
    ///
    /// 参数: 无
    /// 返回: 不包含主对话运行参数的设置视图
    pub(crate) fn subagent_model_settings(&self) -> SubagentModelSettings {
        SubagentModelSettings {
            defaults: SubagentModelChoice {
                provider_id: self.subagent.provider_id.clone(),
                model: self.subagent.model.clone(),
                thinking_level: normalized_thinking(&self.subagent.thinking_level),
            },
            profiles: self
                .resolved_agent_profiles()
                .into_iter()
                .filter(|profile| profile.register_to_main)
                .map(|profile| SubagentModelProfile {
                    selection: profile_selection(self, &profile),
                    id: profile.id,
                    name: profile.name,
                })
                .collect(),
        }
    }

    /// 【配置】【子任务模型】保存共享默认值或一个子任务类型的独立覆盖。
    ///
    /// 参数: `profile_id` 为空时设置共享默认值，`selection` 为模型与思考参数
    /// 返回: 校验与更新结果；非法输入不会部分修改配置
    pub(crate) fn set_subagent_model_choice(
        &mut self,
        profile_id: Option<&str>,
        selection: SubagentModelChoice,
    ) -> Result<()> {
        let selection = SubagentModelChoice {
            provider_id: selection.provider_id.trim().to_string(),
            model: selection.model.trim().to_string(),
            thinking_level: normalized_thinking(&selection.thinking_level),
        };
        validate_selection(self, &selection)?;
        if let Some(id) = profile_id {
            self.resolve_registered_agent(Some(id))
                .with_context(|| format!("subagent profile is not registered: {id}"))?;
            // 1. 显式的继承值也保留，用于覆盖旧 Agent 档案上的模型而不修改主对话
            self.subagent
                .model_overrides
                .insert(id.to_string(), selection);
        } else {
            // 2. 共享设置只修改子任务运行配置
            self.subagent.provider_id = selection.provider_id;
            self.subagent.model = selection.model;
            self.subagent.thinking_level = selection.thinking_level;
        }
        Ok(())
    }
}

/// 返回序列化默认思考值，无参数，返回 auto。
fn auto_thinking() -> String {
    "auto".to_string()
}

/// 归一化思考选择；参数为原始值，空值返回 auto，其余去掉首尾空白。
fn normalized_thinking(value: &str) -> String {
    if value.trim().is_empty() {
        auto_thinking()
    } else {
        value.trim().to_string()
    }
}

/// 【配置】【子任务模型】读取单类型设置，旧 Agent 模型只作为兼容回退。
///
/// 参数: `config` 为应用配置，`profile` 为已解析档案
/// 返回: 单类型的模型与思考选择
fn profile_selection(config: &AppConfig, profile: &AgentProfile) -> SubagentModelChoice {
    config
        .subagent
        .model_overrides
        .get(&profile.id)
        .cloned()
        .unwrap_or_else(|| SubagentModelChoice {
            provider_id: profile.provider_id.clone(),
            model: profile.model.clone(),
            thinking_level: normalized_thinking(&profile.thinking_level),
        })
}

/// 校验模型供应商配对和思考等级；参数为应用配置及选择，返回校验结果。
fn validate_selection(config: &AppConfig, selection: &SubagentModelChoice) -> Result<()> {
    if selection.provider_id.is_empty() != selection.model.is_empty() {
        bail!("subagent model and provider must be configured together");
    }
    if !selection.provider_id.is_empty()
        && !config.provider_model_choices().iter().any(|choice| {
            choice.provider_id == selection.provider_id && choice.model == selection.model
        })
    {
        bail!(
            "subagent model is not enabled: {}/{}",
            selection.provider_id,
            selection.model
        );
    }
    if !matches!(
        selection.thinking_level.as_str(),
        "auto" | "none" | "low" | "medium" | "high" | "xhigh" | "max"
    ) {
        bail!(
            "invalid subagent thinking level: {}",
            selection.thinking_level
        );
    }
    Ok(())
}

/// 【配置】【子任务模型】构造子任务专用的运行配置。
///
/// 参数: `source` 为主会话配置，`profile` 为子任务档案
/// 返回: 独立配置副本，主会话配置保持原值
pub(crate) fn subagent_runtime_config(
    source: &AppConfig,
    profile: &AgentProfile,
) -> Result<AppConfig> {
    let subagent = &source.subagent;
    let choice = profile_selection(source, profile);
    let inherited_thinking = source.provider(None)?.thinking_level.clone();
    let mut config = source.clone();
    let provider_id = if choice.provider_id.is_empty() {
        &subagent.provider_id
    } else {
        &choice.provider_id
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
        let model = if choice.model.is_empty() {
            &subagent.model
        } else {
            &choice.model
        };
        if !model.is_empty() {
            provider.default_model = model.clone();
        }
        let thinking = if choice.thinking_level.is_empty() || choice.thinking_level == "auto" {
            &subagent.thinking_level
        } else {
            &choice.thinking_level
        };
        if !thinking.is_empty() && thinking != "auto" {
            provider.thinking_level = thinking.clone();
        } else {
            provider.thinking_level = inherited_thinking;
        }
    }
    config.provider(None)?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 跨供应商选择模型时，auto 仍按界面承诺继承主对话或共享思考等级。
    #[test]
    fn subagent_thinking_inheritance_survives_provider_switch() {
        let mut config = AppConfig::default();
        let main_id = config.active_provider.clone();
        config
            .providers
            .iter_mut()
            .find(|provider| provider.id == main_id)
            .unwrap()
            .thinking_level = "high".into();
        let mut alternate = config.provider(None).unwrap().clone();
        alternate.id = "alternate".into();
        alternate.thinking_level = "low".into();
        config.providers.push(alternate.clone());
        config
            .set_subagent_model_choice(
                None,
                SubagentModelChoice {
                    provider_id: alternate.id.clone(),
                    model: alternate.default_model.clone(),
                    ..Default::default()
                },
            )
            .unwrap();
        let profile = config.resolve_registered_agent(Some("explore")).unwrap();
        assert_eq!(
            subagent_runtime_config(&config, &profile)
                .unwrap()
                .provider(None)
                .unwrap()
                .thinking_level,
            "high"
        );
        config.subagent.thinking_level = "medium".into();
        assert_eq!(
            subagent_runtime_config(&config, &profile)
                .unwrap()
                .provider(None)
                .unwrap()
                .thinking_level,
            "medium"
        );
        assert_eq!(config.provider(None).unwrap().thinking_level, "high");
    }

    /// 【配置】【子任务模型测试】只修改共享思考等级时也必须应用到客户端配置。
    #[test]
    fn regression_thinking_only_subagent_configuration_is_applied() {
        let mut config = AppConfig::default();
        config.subagent.thinking_level = "high".into();
        let profile = config.resolve_registered_agent(Some("explore")).unwrap();
        let resolved = subagent_runtime_config(&config, &profile).unwrap();
        assert_eq!(resolved.provider(None).unwrap().thinking_level, "high");
        assert_ne!(config.provider(None).unwrap().thinking_level, "high");
    }

    /// 子任务单独覆盖优先于共享设置，并且不会修改主对话档案。
    #[test]
    fn subagent_overrides_are_isolated_from_main_agent_profiles() {
        let mut config = AppConfig::default();
        let profiles = config.resolved_agent_profiles();
        let provider = config.providers[0].clone();
        config
            .set_subagent_model_choice(
                None,
                SubagentModelChoice {
                    thinking_level: "low".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        config
            .set_subagent_model_choice(
                Some("explore"),
                SubagentModelChoice {
                    provider_id: provider.id.clone(),
                    model: provider.default_model.clone(),
                    thinking_level: "high".into(),
                },
            )
            .unwrap();
        assert_eq!(config.resolved_agent_profiles(), profiles);
        let explore = config.resolve_registered_agent(Some("explore")).unwrap();
        assert_eq!(
            subagent_runtime_config(&config, &explore)
                .unwrap()
                .provider(None)
                .unwrap()
                .thinking_level,
            "high"
        );
        config
            .set_subagent_model_choice(Some("explore"), SubagentModelChoice::default())
            .unwrap();
        assert_eq!(
            subagent_runtime_config(&config, &explore)
                .unwrap()
                .provider(None)
                .unwrap()
                .thinking_level,
            "low"
        );
    }

    /// 非法供应商或不可委派档案不会部分写入子任务设置。
    #[test]
    fn invalid_subagent_selection_leaves_settings_unchanged() {
        let mut config = AppConfig::default();
        let before = config.subagent.clone();
        assert!(config
            .set_subagent_model_choice(
                None,
                SubagentModelChoice {
                    provider_id: "missing".into(),
                    model: "model".into(),
                    ..Default::default()
                }
            )
            .is_err());
        assert!(config
            .set_subagent_model_choice(Some("unknown"), SubagentModelChoice::default())
            .is_err());
        assert_eq!(config.subagent, before);
    }

    /// 子任务覆盖保存并重新加载后仍然生效。
    #[test]
    fn subagent_model_selection_survives_configuration_reload() {
        let temp = tempfile::tempdir().unwrap();
        let paths = crate::paths::SaiPaths::for_tests(temp.path());
        let mut config = AppConfig::default();
        config
            .set_subagent_model_choice(
                Some("explore"),
                SubagentModelChoice {
                    thinking_level: "none".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        config.save(&paths).unwrap();
        let reloaded = AppConfig::load(&paths).unwrap();
        assert_eq!(
            reloaded.subagent.model_overrides["explore"].thinking_level,
            "none"
        );
    }
}
