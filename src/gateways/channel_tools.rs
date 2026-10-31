use super::channel_delivery::{send_qq_message, send_weixin_message};
use super::channel_target::resolve_latest_target;
pub(crate) use super::channel_target::{resolve_channel_target, ActiveChannelTarget};
use crate::config::AppConfig;
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use crate::tools::{ToolRegistry, ToolSpec};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

/// 【渠道消息】【通用注册】参数为 registry、paths、config，返回无。
/// 只在未注册时挂载普通会话入口，保留网关已绑定的入站目标
pub(crate) fn register_unbound_channel_message_tool(
    registry: &mut ToolRegistry,
    paths: &SaiPaths,
    config: &AppConfig,
) {
    if !registry.contains("send_channel_message") {
        register_channel_message_tool(
            registry,
            paths.clone(),
            config.clone(),
            ActiveChannelTarget::Unbound,
        );
    }
}

/// 注册统一渠道发送工具。
///
/// 参数:
/// - `registry`: 工具注册表
/// - `paths`: Sai 路径
/// - `config`: 应用配置
/// - `current`: 当前入站消息对应渠道目标
///
/// 返回:
/// - 无
pub(crate) fn register_channel_message_tool(
    registry: &mut ToolRegistry,
    paths: SaiPaths,
    config: AppConfig,
    current: ActiveChannelTarget,
) {
    registry.register(
        ToolSpec::new(
            "send_channel_message",
            "Send text or a local media file to a channel. In a gateway chat, channel=current preserves that chat's recipient. In a terminal, CLI or subagent, current/default sends to the most recent Weixin chat. Specify qq or weixin to use that channel's most recent chat. A recent chat must exist; this tool never guesses a recipient.",
            json!({
                "type": "object",
                "properties": {
                    "channel": {
                        "type": "string",
                        "description": "Target channel: current/default, qq, or weixin. Unbound sessions default to the latest Weixin chat.",
                        "enum": ["current", "default", "qq", "weixin"]
                    },
                    "kind": {
                        "type": "string",
                        "description": "Message kind: text, image, file, or video.",
                        "enum": ["text", "image", "file", "video"]
                    },
                    "text": {
                        "type": "string",
                        "description": "Text message content, or optional text sent before media."
                    },
                    "path": {
                        "type": "string",
                        "description": "Local file path for image, file, or video."
                    },
                    "caption": {
                        "type": "string",
                        "description": "Optional media caption. If text is also provided, text takes precedence."
                    }
                },
                "required": ["kind"],
                "additionalProperties": false
            }),
            move |args| {
                let paths = paths.clone();
                let config = config.clone();
                let current = current.clone();
                async move { send_channel_message(args, paths, config, current).await }
            },
        )
        .writes(),
    );
}

/// 执行统一渠道发送工具。
///
/// 参数:
/// - `args`: 工具参数
/// - `paths`: Sai 路径
/// - `config`: 应用配置
/// - `current`: 当前入站消息对应渠道目标
///
/// 返回:
/// - JSON 格式发送结果
async fn send_channel_message(
    args: Value,
    paths: SaiPaths,
    config: AppConfig,
    current: ActiveChannelTarget,
) -> Result<String> {
    let channel = optional_string(&args, "channel").unwrap_or_else(|| "current".to_string());
    let kind = required_string(&args, "kind")?;
    let text = optional_string(&args, "text").or_else(|| optional_string(&args, "caption"));
    let path = optional_string(&args, "path").map(|path| crate::tools::fs_path::expand_path(&path));
    let target = resolve_target(&paths, &config, current, &channel).await?;
    match target {
        ActiveChannelTarget::Unbound => bail!("channel target resolution returned no recipient"),
        ActiveChannelTarget::Qq { client, msg_id } => {
            send_qq_message(client, msg_id, &kind, text, path).await
        }
        ActiveChannelTarget::Weixin {
            client,
            to_user_id,
            context_token,
        } => send_weixin_message(client, to_user_id, context_token, &kind, text, path).await,
    }
}

/// 解析发送目标。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 应用配置
/// - `current`: 当前渠道目标
/// - `channel`: 目标渠道
///
/// 返回:
/// - 实际发送目标
async fn resolve_target(
    paths: &SaiPaths,
    config: &AppConfig,
    current: ActiveChannelTarget,
    channel: &str,
) -> Result<ActiveChannelTarget> {
    let channel = channel.trim().to_ascii_lowercase();
    if matches!(current, ActiveChannelTarget::Unbound) {
        let requested = match channel.as_str() {
            "" | "current" | "default" => "weixin",
            "qq" | "weixin" | "wechat" => channel.as_str(),
            value => bail!("{}: {value}", t("unsupported channel", "不支持的渠道")),
        };
        return resolve_latest_target(paths, config, requested).await;
    }
    match channel.as_str() {
        "" | "current" | "default" => Ok(current),
        // 1. 【渠道消息】【目标隔离】同渠道名称绑定本次入站目标，不读取其他并发会话的最近目标
        "qq" if matches!(current, ActiveChannelTarget::Qq { .. }) => Ok(current),
        "weixin" | "wechat" if matches!(current, ActiveChannelTarget::Weixin { .. }) => Ok(current),
        "qq" | "weixin" | "wechat" => resolve_latest_target(paths, config, &channel).await,
        value => bail!("{}: {value}", t("unsupported channel", "不支持的渠道")),
    }
}

/// 读取必填字符串参数。
///
/// 参数:
/// - `args`: 工具参数
/// - `key`: 字段名
///
/// 返回:
/// - 字符串值
fn required_string(args: &Value, key: &str) -> Result<String> {
    optional_string(args, key).with_context(|| format!("{key} {}", t("is required", "不能为空")))
}

/// 读取可选字符串参数。
///
/// 参数:
/// - `args`: 工具参数
/// - `key`: 字段名
///
/// 返回:
/// - 可选字符串值
fn optional_string(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
#[path = "channel_tools_integration_tests.rs"]
mod integration_tests;

#[cfg(test)]
mod tests {
    use super::super::channel_delivery::{required_path, required_text};
    use super::super::channel_target::non_empty_config;
    use super::super::qq_official::{QqOfficialClient, QqTargetKind};
    use super::*;

    /// 【渠道消息】【目标回归】显式指定当前渠道也沿用本次入站目标，不解析全局最近会话。
    /// @returns 无；无参数
    #[tokio::test]
    async fn explicit_current_channel_keeps_request_target() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();
        let paths = SaiPaths {
            config_dir: root.join("config"),
            config_file: root.join("config/config.jsonc"),
            secrets_file: root.join("config/secrets.jsonc"),
            skills_dir: root.join("skills"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            state_dir: root.join("state"),
            pictures_dir: root.join("pictures"),
            fish_hook_file: root.join("fish"),
            bash_hook_file: root.join("bash"),
            zsh_hook_file: root.join("zsh"),
            powershell_hook_file: root.join("ps"),
        };
        let current = ActiveChannelTarget::Qq {
            client: QqOfficialClient::new(
                "http://127.0.0.1:1".into(),
                "fixture".into(),
                QqTargetKind::User,
                "request-user".into(),
            ),
            msg_id: Some("request-message".into()),
        };
        let target = resolve_target(&paths, &AppConfig::default(), current, "qq")
            .await
            .unwrap();
        assert!(
            matches!(target, ActiveChannelTarget::Qq { msg_id: Some(id), .. } if id == "request-message")
        );
    }

    /// 验证可选字符串参数会去除首尾空白并忽略空值。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn optional_string_trims_and_ignores_empty_values() {
        let args = json!({
            "channel": " qq ",
            "text": "   ",
            "count": 1
        });

        assert_eq!(optional_string(&args, "channel").as_deref(), Some("qq"));
        assert_eq!(optional_string(&args, "text"), None);
        assert_eq!(optional_string(&args, "count"), None);
    }

    /// 验证必填字符串参数缺失时返回错误。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn required_string_rejects_missing_values() {
        let err = required_string(&json!({}), "kind").unwrap_err();

        assert!(err
            .to_string()
            .contains(&format!("kind {}", t("is required", "不能为空"))));
    }

    /// 验证文本消息必须提供非空文本。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn required_text_rejects_blank_text() {
        let err = required_text(Some("   ".to_string())).unwrap_err();

        assert!(err.to_string().contains(t(
            "text is required for text messages",
            "文本消息必须提供 text"
        )));
    }

    /// 验证媒体消息必须提供本机路径。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn required_path_rejects_missing_path() {
        let err = required_path(None).unwrap_err();

        assert!(err.to_string().contains(t(
            "path is required for media messages",
            "媒体消息必须提供 path"
        )));
    }

    /// 验证非空配置读取会去除空白。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn non_empty_config_trims_values() {
        assert_eq!(
            non_empty_config(" https://example.invalid "),
            Some("https://example.invalid".to_string())
        );
        assert_eq!(non_empty_config("  "), None);
    }
}
