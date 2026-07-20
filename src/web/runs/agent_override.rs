use crate::config::{AgentSurface, AppConfig};
use anyhow::Result;

/// 把指定 Agent 档案应用到单轮内存配置。
///
/// 参数:
/// - `config`: 当前应用配置
/// - `agent_id`: 主界面选择的 Agent 标识
///
/// 返回:
/// - 已应用系统提示词、工具和 skills 策略的配置
pub(super) fn apply_agent_override(config: AppConfig, agent_id: Option<&str>) -> Result<AppConfig> {
    crate::config::apply_agent_override(config, agent_id, AgentSurface::Web)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AgentProfile, DEFAULT_AGENT_ID};

    #[test]
    fn keeps_existing_runtime_for_virtual_default_agent() {
        let config = AppConfig::default();
        let resolved = apply_agent_override(config, Some(DEFAULT_AGENT_ID)).unwrap();
        assert!(resolved.agent_runtime.is_none());
        assert!(resolved.system_prompt_file.is_some());
    }

    #[test]
    fn applies_configured_agent_capabilities() {
        let mut config = AppConfig::default();
        config.agents.push(AgentProfile {
            id: "reviewer".to_string(),
            name: "审查".to_string(),
            system_prompt: "只审查代码".to_string(),
            enabled_tools: vec!["read_file".to_string()],
            skills_full: vec!["code-review".to_string()],
            skills_named: vec!["research".to_string()],
            ..AgentProfile::default()
        });
        let resolved = apply_agent_override(config, Some("reviewer")).unwrap();
        assert_eq!(resolved.system_prompt.as_deref(), Some("只审查代码"));
        let runtime = resolved.agent_runtime.unwrap();
        assert_eq!(runtime.enabled_tools, vec!["read_file"]);
        assert_eq!(runtime.skills_full, vec!["code-review"]);
        assert_eq!(runtime.skills_named, vec!["research"]);
    }

    #[test]
    fn rejects_unknown_agent() {
        let error = apply_agent_override(AppConfig::default(), Some("missing")).unwrap_err();
        assert!(error.to_string().contains("agent not found"));
    }

    #[test]
    fn falls_back_to_default_agent_when_id_absent() {
        let mut config = AppConfig::default();
        config.agents.push(AgentProfile {
            id: "writer".to_string(),
            name: "写作".to_string(),
            system_prompt: "专注写作".to_string(),
            enabled_tools: vec!["read_file".to_string()],
            skills_full: Vec::new(),
            skills_named: Vec::new(),
            ..AgentProfile::default()
        });
        config.default_agent = Some("writer".to_string());
        let resolved = apply_agent_override(config, None).unwrap();
        assert_eq!(resolved.system_prompt.as_deref(), Some("专注写作"));
    }

    #[test]
    fn explicit_agent_overrides_default_agent() {
        let mut config = AppConfig::default();
        config.agents.push(AgentProfile {
            id: "writer".to_string(),
            name: "写作".to_string(),
            system_prompt: "专注写作".to_string(),
            enabled_tools: Vec::new(),
            skills_full: Vec::new(),
            skills_named: Vec::new(),
            ..AgentProfile::default()
        });
        config.default_agent = Some("writer".to_string());
        // 显式传入虚拟默认 agent 时,应忽略 default_agent 回退
        let resolved = apply_agent_override(config, Some(DEFAULT_AGENT_ID)).unwrap();
        assert!(resolved.agent_runtime.is_none());
    }

    #[test]
    fn agent_system_prompt_overrides_runtime_config() {
        let mut config = AppConfig::default();
        config.agents.push(AgentProfile {
            id: "code-agent".to_string(),
            name: "代码".to_string(),
            system_prompt: "code-agent-content".to_string(),
            register_to_main: true,
            ..AgentProfile::default()
        });
        let resolved = apply_agent_override(config, Some("code-agent")).unwrap();
        assert_eq!(resolved.system_prompt.as_deref(), Some("code-agent-content"));
        assert!(resolved.system_prompt_file.is_none());
    }
}
