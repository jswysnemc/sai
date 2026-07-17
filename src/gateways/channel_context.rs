use crate::gateways::qq_official::QqTargetKind;
use crate::paths::SaiPaths;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "channel")]
pub(crate) enum ChannelContext {
    #[serde(rename = "qq")]
    Qq {
        gateway: String,
        target_kind: String,
        target_id: String,
        msg_id: Option<String>,
    },
    #[serde(rename = "weixin")]
    Weixin {
        gateway: String,
        to_user_id: String,
        context_token: Option<String>,
    },
}

impl ChannelContext {
    /// 返回渠道名称。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 渠道名称
    pub(crate) fn channel(&self) -> &'static str {
        match self {
            Self::Qq { .. } => "qq",
            Self::Weixin { .. } => "weixin",
        }
    }

    /// 创建 QQ 渠道上下文。
    ///
    /// 参数:
    /// - `target_kind`: QQ 目标类型
    /// - `target_id`: QQ 目标 ID
    /// - `msg_id`: 当前消息 ID
    ///
    /// 返回:
    /// - 渠道上下文
    pub(crate) fn qq(target_kind: QqTargetKind, target_id: String, msg_id: Option<String>) -> Self {
        Self::Qq {
            gateway: "qq-bot".to_string(),
            target_kind: qq_target_kind_name(target_kind).to_string(),
            target_id,
            msg_id,
        }
    }

    /// 创建微信渠道上下文。
    ///
    /// 参数:
    /// - `to_user_id`: 接收方用户 ID
    /// - `context_token`: 入站上下文 token
    ///
    /// 返回:
    /// - 渠道上下文
    pub(crate) fn weixin(to_user_id: String, context_token: Option<String>) -> Self {
        Self::Weixin {
            gateway: "weixin-server".to_string(),
            to_user_id,
            context_token,
        }
    }

    /// 返回入站消息短标记。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 短标记文本
    pub(crate) fn inbound_marker(&self) -> String {
        match self {
            Self::Qq {
                gateway,
                target_kind,
                ..
            } => format!("[channel=qq gateway={gateway} target={target_kind}]"),
            Self::Weixin { gateway, .. } => {
                format!("[channel=weixin gateway={gateway}]")
            }
        }
    }

    /// 返回当前渠道对应的系统提示词。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 渠道系统提示词
    pub(crate) fn system_prompt(&self) -> &'static str {
        match self {
            Self::Qq { .. } => super::qq_bot::prompt::channel_prompt(),
            Self::Weixin { .. } => super::weixin_bot::prompt::channel_prompt(),
        }
    }
}

/// 保存最近渠道上下文。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `context`: 渠道上下文
///
/// 返回:
/// - 保存是否成功
pub(crate) fn save_latest_channel_context(
    paths: &SaiPaths,
    context: &ChannelContext,
) -> Result<()> {
    let mut contexts = load_context_map(paths)?;
    contexts.insert(context.channel().to_string(), context.clone());
    let file = contexts_file(paths);
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        file,
        format!("{}\n", serde_json::to_string_pretty(&contexts)?),
    )?;
    Ok(())
}

/// 读取最近渠道上下文。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `channel`: 渠道名称
///
/// 返回:
/// - 最近渠道上下文
pub(crate) fn load_latest_channel_context(
    paths: &SaiPaths,
    channel: &str,
) -> Result<Option<ChannelContext>> {
    let contexts = load_context_map(paths)?;
    Ok(contexts.get(channel.trim()).cloned())
}

/// 保存指定网关会话的渠道上下文。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `workspace_path`: 网关工作区
/// - `session_id`: 网关会话标识
/// - `context`: 渠道上下文
///
/// 返回:
/// - 保存是否成功
pub(crate) fn save_session_channel_context(
    paths: &SaiPaths,
    workspace_path: &Path,
    session_id: &str,
    context: &ChannelContext,
) -> Result<()> {
    let (_, state_dir) =
        crate::state::state_dir_for_workspace_session(paths, workspace_path, session_id)?;
    std::fs::write(
        state_dir.join("channel-context.json"),
        format!("{}\n", serde_json::to_string_pretty(context)?),
    )?;
    Ok(())
}

/// 读取指定网关会话的渠道上下文。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `workspace_path`: 网关工作区
/// - `session_id`: 网关会话标识
///
/// 返回:
/// - 渠道上下文，会话尚未绑定时返回空
pub(crate) fn load_session_channel_context(
    paths: &SaiPaths,
    workspace_path: &Path,
    session_id: &str,
) -> Result<Option<ChannelContext>> {
    let (_, state_dir) =
        crate::state::state_dir_for_workspace_session(paths, workspace_path, session_id)?;
    let file = state_dir.join("channel-context.json");
    if !file.exists() {
        return Ok(None);
    }
    Ok(Some(serde_json::from_slice(&std::fs::read(file)?)?))
}

/// 读取所有最近渠道上下文。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - 渠道上下文映射
fn load_context_map(paths: &SaiPaths) -> Result<BTreeMap<String, ChannelContext>> {
    let file = contexts_file(paths);
    if !file.exists() {
        return Ok(BTreeMap::new());
    }
    let raw = std::fs::read_to_string(file)?;
    Ok(serde_json::from_str(&raw)?)
}

/// 返回渠道上下文状态文件。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - 状态文件路径
fn contexts_file(paths: &SaiPaths) -> PathBuf {
    paths
        .state_dir
        .join("gateways")
        .join("channel-contexts.json")
}

/// 返回 QQ 目标类型名称。
///
/// 参数:
/// - `target_kind`: QQ 目标类型
///
/// 返回:
/// - 目标类型文本
fn qq_target_kind_name(target_kind: QqTargetKind) -> &'static str {
    match target_kind {
        QqTargetKind::User => "user",
        QqTargetKind::Group => "group",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证 QQ 入站标记包含渠道、网关和目标类型。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn qq_inbound_marker_includes_channel_gateway_and_target() {
        let context = ChannelContext::qq(
            QqTargetKind::Group,
            "group-openid".to_string(),
            Some("msg-1".to_string()),
        );

        assert_eq!(
            context.inbound_marker(),
            "[channel=qq gateway=qq-bot target=group]"
        );
    }

    /// 验证微信入站标记包含渠道和网关。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn weixin_inbound_marker_includes_channel_and_gateway() {
        let context = ChannelContext::weixin("wx-user".to_string(), Some("ctx".to_string()));

        assert_eq!(
            context.inbound_marker(),
            "[channel=weixin gateway=weixin-server]"
        );
    }

    /// 验证最近渠道上下文可以保存并读取。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn saves_and_loads_latest_channel_context() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        let context = ChannelContext::qq(
            QqTargetKind::User,
            "user-openid".to_string(),
            Some("msg-2".to_string()),
        );

        save_latest_channel_context(&paths, &context).unwrap();
        let loaded = load_latest_channel_context(&paths, "qq").unwrap().unwrap();

        match loaded {
            ChannelContext::Qq {
                target_kind,
                target_id,
                msg_id,
                ..
            } => {
                assert_eq!(target_kind, "user");
                assert_eq!(target_id, "user-openid");
                assert_eq!(msg_id.as_deref(), Some("msg-2"));
            }
            ChannelContext::Weixin { .. } => panic!("expected QQ context"),
        }
    }

    /// 创建测试用 Sai 路径。
    ///
    /// 参数:
    /// - `root`: 临时目录根路径
    ///
    /// 返回:
    /// - 测试用路径配置
    fn test_paths(root: PathBuf) -> SaiPaths {
        SaiPaths {
            config_dir: root.join("config"),
            config_file: root.join("config").join("config.jsonc"),
            secrets_file: root.join("config").join("secrets.jsonc"),
            skills_dir: root.join("config").join("skills"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            state_dir: root.join("state"),
            pictures_dir: root.join("pictures"),
            fish_hook_file: root.join("fish").join("sai.fish"),
            bash_hook_file: root.join("shell").join("bash-hook.sh"),
            zsh_hook_file: root.join("shell").join("zsh-hook.zsh"),
            powershell_hook_file: root.join("shell").join("powershell-hook.ps1"),
        }
    }
}
