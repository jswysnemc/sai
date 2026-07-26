use super::ExternalTurnEngine;
use crate::config::AgentEngineConfig;
use std::fmt;

/// 外部内核不可用的原因。
///
/// 内核不可用时会话必须能继续，因此这里只描述原因，由调用方决定回退策略。
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum ExternalEngineError {
    /// 选择了自定义内核但没有给出启动命令
    MissingCommand,
}

impl fmt::Display for ExternalEngineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCommand => write!(
                formatter,
                "{}",
                crate::i18n::text(
                    "custom agent engine requires agent.acp.command",
                    "自定义对话内核需要配置 agent.acp.command",
                )
            ),
        }
    }
}

/// 按配置构造外部对话内核。
///
/// 参数:
/// - `config`: 内核配置
/// - `governance`: 治理句柄，外部内核的落盘与执行都要过它
///
/// 返回:
/// - 使用原生内核时返回 `Ok(None)`；外部内核不可用时返回错误
pub(crate) fn build_external_engine(
    config: &AgentEngineConfig,
    governance: crate::acp::AcpGovernance,
) -> Result<Option<Box<dyn ExternalTurnEngine>>, ExternalEngineError> {
    if !config.engine.is_external() {
        return Ok(None);
    }
    // 1. 命令缺失属于配置问题，先于协议实现报出来
    if config.resolved_command().is_none() {
        return Err(ExternalEngineError::MissingCommand);
    }
    // 2. 命令齐备即认为可用；进程启动与握手推迟到首轮对话，
    //    避免每次构造 Agent（配置页预览、上下文估算等）都拉起一个外部进程
    Ok(Some(Box::new(super::lazy::LazyAcpEngine::new(
        config.engine.as_str().to_string(),
        config.clone(),
        governance,
    ))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AgentEngineKind, AppConfig};

    /// 构造测试用治理句柄。
    ///
    /// 返回:
    /// - 不绑定权限配置的句柄
    fn test_governance() -> crate::acp::AcpGovernance {
        crate::acp::AcpGovernance::new(
            std::path::PathBuf::from("/tmp"),
            None,
            AppConfig::default(),
            "test-session".to_string(),
        )
    }

    #[test]
    fn native_engine_needs_no_external_process() {
        let config = AgentEngineConfig::default();
        assert!(matches!(build_external_engine(&config, test_governance()), Ok(None)));
    }

    /// 取出构建错误；内核 trait object 不实现 Debug，因此不能用 unwrap_err。
    ///
    /// 参数:
    /// - `config`: 内核配置
    ///
    /// 返回:
    /// - 构建失败时的错误
    fn build_error(config: &AgentEngineConfig) -> ExternalEngineError {
        match build_external_engine(config, test_governance()) {
            Ok(_) => panic!("expected the engine build to fail"),
            Err(error) => error,
        }
    }

    #[test]
    fn custom_engine_without_command_reports_missing_command() {
        let config: AgentEngineConfig = serde_json::from_str(r#"{"engine":"custom"}"#).unwrap();
        assert_eq!(build_error(&config), ExternalEngineError::MissingCommand);
    }

    /// 用户配置里写的是 claude_code 时，必须真的构造出外部内核。
    ///
    /// 这条用例针对的疑问是「切了配置但看起来还是原生」：
    /// 若分流在这里就退回 None，后面一切都还是 sai 自己的循环。
    #[test]
    fn claude_code_config_produces_an_external_engine() {
        let config: AgentEngineConfig = serde_json::from_str(
            r#"{"engine":"claude_code","acp":{"startup_timeout_seconds":90}}"#,
        )
        .unwrap();
        assert!(config.engine.is_external());
        assert!(matches!(
            build_external_engine(&config, test_governance()),
            Ok(Some(_))
        ));
    }

    /// 预置内核命令齐备时应构造出内核；进程要到首轮对话才真正启动。
    #[test]
    fn preset_engine_builds_without_launching_a_process() {
        let config: AgentEngineConfig = serde_json::from_str(r#"{"engine":"codex"}"#).unwrap();
        match build_external_engine(&config, test_governance()) {
            Ok(Some(engine)) => assert_eq!(engine.name(), AgentEngineKind::Codex.as_str()),
            Ok(None) => panic!("codex is an external engine"),
            Err(error) => panic!("unexpected build failure: {error}"),
        }
    }
}
