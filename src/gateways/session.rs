use super::channel_context::{save_session_channel_context, ChannelContext};
use super::workspace::gateway_workspace_path;
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::Result;
use sha2::{Digest, Sha256};

/// 确保渠道目标对应的稳定会话存在并保存会话级渠道上下文。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `context`: 当前渠道上下文
///
/// 返回:
/// - 稳定会话标识
pub(crate) fn ensure_gateway_session(paths: &SaiPaths, context: &ChannelContext) -> Result<String> {
    let workspace = gateway_workspace_path(paths);
    std::fs::create_dir_all(&workspace)?;
    let session_id = gateway_session_id(context);
    crate::state::ensure_workspace_session(
        paths,
        &workspace,
        &session_id,
        &gateway_session_title(context),
    )?;
    save_session_channel_context(paths, &workspace, &session_id, context)?;
    Ok(session_id)
}

/// 根据渠道目标生成稳定会话标识。
///
/// 参数:
/// - `context`: 渠道上下文
///
/// 返回:
/// - 稳定会话标识
fn gateway_session_id(context: &ChannelContext) -> String {
    let (channel, target) = match context {
        ChannelContext::Qq {
            target_kind,
            target_id,
            ..
        } => ("qq", format!("{target_kind}:{target_id}")),
        ChannelContext::Weixin { to_user_id, .. } => ("weixin", to_user_id.clone()),
    };
    let mut hasher = Sha256::new();
    hasher.update(target.as_bytes());
    let digest = format!("{:x}", hasher.finalize());
    format!("gateway_{channel}_{}", &digest[..16])
}

/// 返回渠道会话标题。
///
/// 参数:
/// - `context`: 渠道上下文
///
/// 返回:
/// - Web 会话标题
fn gateway_session_title(context: &ChannelContext) -> String {
    match context {
        ChannelContext::Qq {
            target_kind,
            target_id,
            ..
        } => format!(
            "QQ {} · {}",
            qq_target_label(target_kind),
            compact_target(target_id)
        ),
        ChannelContext::Weixin { to_user_id, .. } => {
            format!("{} · {}", t("Weixin", "微信"), compact_target(to_user_id))
        }
    }
}

/// 返回 QQ 目标类型本地化名称。
///
/// 参数:
/// - `target_kind`: QQ 目标类型文本
///
/// 返回:
/// - 本地化名称
fn qq_target_label(target_kind: &str) -> &'static str {
    match target_kind {
        "group" => t("group", "群聊"),
        _ => t("private", "私聊"),
    }
}

/// 压缩目标标识用于界面标题。
///
/// 参数:
/// - `target`: 原始目标标识
///
/// 返回:
/// - 最多十二个字符的标识
fn compact_target(target: &str) -> String {
    let chars = target.chars().collect::<Vec<_>>();
    if chars.len() <= 12 {
        return target.to_string();
    }
    format!(
        "{}…{}",
        chars[..6].iter().collect::<String>(),
        chars[chars.len() - 4..].iter().collect::<String>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateways::qq_official::QqTargetKind;
    use std::path::PathBuf;

    /// 创建网关会话测试路径。
    fn test_paths(root: PathBuf) -> SaiPaths {
        SaiPaths {
            config_dir: root.join("config"),
            config_file: root.join("config/config.jsonc"),
            secrets_file: root.join("config/secrets.jsonc"),
            skills_dir: root.join("config/skills"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            state_dir: root.join("state"),
            pictures_dir: root.join("pictures"),
            fish_hook_file: root.join("fish/sai.fish"),
            bash_hook_file: root.join("shell/bash-hook.sh"),
            zsh_hook_file: root.join("shell/zsh-hook.zsh"),
            powershell_hook_file: root.join("shell/powershell-hook.ps1"),
        }
    }

    #[test]
    /// 验证同一渠道目标生成稳定会话标识。
    fn gateway_session_id_is_stable_per_target() {
        let first = ChannelContext::qq(QqTargetKind::User, "user-a".to_string(), None);
        let second = ChannelContext::qq(QqTargetKind::User, "user-a".to_string(), None);
        let third = ChannelContext::qq(QqTargetKind::User, "user-b".to_string(), None);

        assert_eq!(gateway_session_id(&first), gateway_session_id(&second));
        assert_ne!(gateway_session_id(&first), gateway_session_id(&third));
    }

    #[test]
    /// 验证网关会话可通过现有工作区会话列表读取。
    fn gateway_session_is_visible_in_workspace_session_list() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        let context = ChannelContext::qq(
            QqTargetKind::Group,
            "group-openid".to_string(),
            Some("message-id".to_string()),
        );

        let session_id = ensure_gateway_session(&paths, &context).unwrap();
        let workspace = gateway_workspace_path(&paths);
        let sessions = crate::state::list_sessions_for_workspace(&paths, &workspace).unwrap();
        let loaded = crate::gateways::channel_context::load_session_channel_context(
            &paths,
            &workspace,
            &session_id,
        )
        .unwrap();

        assert!(sessions.iter().any(|session| session.id == session_id));
        assert!(matches!(loaded, Some(ChannelContext::Qq { .. })));
    }
}
