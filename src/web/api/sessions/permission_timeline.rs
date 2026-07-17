use crate::state::{SessionTimelineTurn, StateStore, TimelinePermissionDecision};
use anyhow::Result;
use serde::Deserialize;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};

/// 权限审计日志中用于恢复历史消息流的字段。
#[derive(Deserialize)]
struct StoredPermissionEvent {
    tool: String,
    decision: String,
    arguments: Value,
    detail: Option<String>,
}

/// 将权限审计决定关联到结构化工具时间线。
///
/// 参数:
/// - `store`: 当前会话状态存储
/// - `timeline`: 待补充权限决定的会话时间线
///
/// 返回:
/// - 审计日志读取和关联结果
pub(super) fn attach_permission_decisions(
    store: &StateStore,
    timeline: &mut [SessionTimelineTurn],
) -> Result<()> {
    let path = store.state_dir().join("permission-audit.jsonl");
    if !path.exists() {
        return Ok(());
    }
    // 1. 按工具和参数配对 requested 与 allowed/denied 事件
    let content = std::fs::read_to_string(path)?;
    let mut pending = HashMap::<String, usize>::new();
    let mut resolved = HashMap::<String, VecDeque<TimelinePermissionDecision>>::new();
    for event in content
        .lines()
        .filter_map(|line| serde_json::from_str::<StoredPermissionEvent>(line).ok())
    {
        let key = permission_key(&event.tool, &event.arguments);
        match event.decision.as_str() {
            "requested" => *pending.entry(key).or_default() += 1,
            "approved" | "denied" if pending.get(&key).copied().unwrap_or_default() > 0 => {
                if let Some(count) = pending.get_mut(&key) {
                    *count -= 1;
                }
                resolved
                    .entry(key)
                    .or_default()
                    .push_back(TimelinePermissionDecision {
                        decision: if event.decision == "approved" {
                            "allow".to_string()
                        } else {
                            "deny".to_string()
                        },
                        reply: (event.decision == "denied")
                            .then_some(event.detail)
                            .flatten()
                            .filter(|value| !value.trim().is_empty()),
                    });
            }
            _ => {}
        }
    }
    // 2. 按对话和工具顺序消费匹配的权限决定
    for tool in timeline.iter_mut().flat_map(|turn| turn.tools.iter_mut()) {
        let arguments = serde_json::from_str(&tool.arguments).unwrap_or(Value::Null);
        let key = permission_key(&tool.name, &arguments);
        tool.permission = resolved.get_mut(&key).and_then(VecDeque::pop_front);
    }
    Ok(())
}

/// 创建工具名称和规范参数组成的稳定匹配键。
///
/// 参数:
/// - `tool`: 工具名称
/// - `arguments`: 已解析工具参数
///
/// 返回:
/// - 权限审计和工具时间线共用的匹配键
fn permission_key(tool: &str, arguments: &Value) -> String {
    format!(
        "{tool}\n{}",
        serde_json::to_string(arguments).unwrap_or_default()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::SaiPaths;
    use crate::permission::{AuditDecision, PermissionAuditLog};
    use std::path::PathBuf;

    /// 创建权限时间线测试使用的隔离路径。
    ///
    /// 参数:
    /// - `root`: 临时根目录
    ///
    /// 返回:
    /// - Sai 路径集合
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

    /// 验证完成运行后仍能从审计日志恢复拒绝决定。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn attaches_denied_permission_to_completed_timeline() {
        let temp = tempfile::tempdir().unwrap();
        let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
        store.start_turn("turn", "inspect").unwrap();
        store
            .record_tool_call_started("turn", 0, "call", "edit_file", r#"{"path":"a.rs"}"#)
            .unwrap();
        store
            .record_tool_result_completed("turn", "call", false, "拒绝", None, None, 2)
            .unwrap();
        store.complete_turn("turn", "done", None).unwrap();
        let audit = PermissionAuditLog::new(
            store.state_dir().join("permission-audit.jsonl"),
            store.session_id(),
        );
        let arguments = serde_json::json!({"path":"a.rs"});
        audit
            .append(
                "audited",
                "edit_file",
                AuditDecision::Requested,
                &arguments,
                None,
            )
            .unwrap();
        audit
            .append(
                "audited",
                "edit_file",
                AuditDecision::Denied,
                &arguments,
                Some("保留文件"),
            )
            .unwrap();
        let mut timeline = store.session_timeline(10).unwrap();

        attach_permission_decisions(&store, &mut timeline).unwrap();

        let permission = timeline[0].tools[0].permission.as_ref().unwrap();
        assert_eq!(permission.decision, "deny");
        assert_eq!(permission.reply.as_deref(), Some("保留文件"));
    }
}
