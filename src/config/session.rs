use super::defaults::{default_thinking_level, is_auto_thinking_level};
use super::AppConfig;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

const ACP_PROVIDER_ID: &str = "__acp__";

/// 【会话】【配置结构】会话创建默认值与自动命名配置。
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct SessionConfig {
    /// 新会话默认供应商；空则使用当前内核默认模型。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub new_session_provider_id: String,
    /// 新会话默认模型；与供应商同时配置。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub new_session_model: String,
    /// 新会话默认思考等级。
    #[serde(
        default = "default_thinking_level",
        skip_serializing_if = "is_auto_thinking_level"
    )]
    pub new_session_thinking_level: String,
    /// 是否在新建会话的首轮自动生成标题。
    #[serde(default = "default_true")]
    pub auto_title_enabled: bool,
    /// 自动标题专用供应商；空则使用当前会话供应商。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub auto_title_provider_id: String,
    /// 自动标题专用模型；空则使用当前会话模型。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub auto_title_model: String,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            new_session_provider_id: String::new(),
            new_session_model: String::new(),
            new_session_thinking_level: default_thinking_level(),
            auto_title_enabled: true,
            auto_title_provider_id: String::new(),
            auto_title_model: String::new(),
        }
    }
}

impl AppConfig {
    /// 【会话】【新会话默认值】校验默认模型与思考等级。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 配置完整且模型适用于当前内核时返回成功
    pub(super) fn validate_new_session_defaults(&self) -> Result<()> {
        let provider_id = self.session.new_session_provider_id.trim();
        let model = self.session.new_session_model.trim();

        // 1. 【会话】【新会话默认值】供应商与模型必须同时留空或同时配置
        match (provider_id.is_empty(), model.is_empty()) {
            (true, true) => {}
            (false, false) if self.agent.engine.is_external() => {
                if provider_id != ACP_PROVIDER_ID {
                    bail!("session.new_session_provider_id must be __acp__ for external engines");
                }
            }
            (false, false) => {
                let configured = self.provider_model_choices().iter().any(|choice| {
                    choice.provider_id == provider_id && choice.model == model
                });
                if !configured {
                    bail!(
                        "session new-session model is not configured: {provider_id}/{model}"
                    );
                }
            }
            _ => bail!(
                "session.new_session_provider_id and session.new_session_model must be provided together"
            ),
        }

        // 2. 【会话】【新会话默认值】思考等级必须使用 Web 运行接口支持的稳定值
        match self.session.new_session_thinking_level.trim() {
            "" | "auto" | "none" | "low" | "medium" | "high" | "xhigh" | "max" => {
                Ok(())
            }
            value => bail!("session.new_session_thinking_level is invalid: {value}"),
        }
    }
}

fn default_true() -> bool {
    true
}
