use crate::paths::SaiPaths;
use crate::tools::command::{BackgroundCommandStore, BackgroundCommandTask};
use crate::tools::subagent_state::list_subagents_for_owner;
use std::path::Path;

/// 仍在运行、关闭会话前需要告诉用户的后台工作。
struct RunningWork {
    subagents: Vec<String>,
    commands: Vec<String>,
}

/// 退出前检查当前会话的后台子智能体和后台命令。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `session_id`: 当前会话标识
/// - `state_dir`: 会话状态目录，用作子智能体归属键
///
/// 返回:
/// - 有运行中的工作时返回提示文本；没有时返回空
pub(super) fn background_exit_notice(
    paths: &SaiPaths,
    session_id: &str,
    state_dir: &Path,
) -> Option<String> {
    let work = running_work(paths, session_id, state_dir);
    if work.subagents.is_empty() && work.commands.is_empty() {
        return None;
    }
    Some(format_notice(&work))
}

/// 收集当前会话仍在运行的后台工作名称。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `session_id`: 会话标识
/// - `state_dir`: 子智能体归属目录
///
/// 返回:
/// - 子智能体描述和后台命令标签
fn running_work(paths: &SaiPaths, session_id: &str, state_dir: &Path) -> RunningWork {
    let owner_key = state_dir.display().to_string();
    let subagents = list_subagents_for_owner(&owner_key)
        .into_iter()
        .filter(|snapshot| snapshot.status == "running")
        .map(|snapshot| {
            let label = snapshot.description.trim();
            if label.is_empty() {
                snapshot.subagent_type
            } else {
                label.to_string()
            }
        })
        .collect();
    let commands = BackgroundCommandStore::new(paths.state_dir.clone())
        .load()
        .unwrap_or_default()
        .into_iter()
        .filter(|task| task.owned_by_session(session_id) && task.status == "running")
        .map(command_label)
        .collect();
    RunningWork { subagents, commands }
}

/// 取后台命令的短标签。
///
/// 参数:
/// - `task`: 后台命令
///
/// 返回:
/// - 优先使用标签，否则使用命令前 48 个字符
fn command_label(task: BackgroundCommandTask) -> String {
    let label = task.label.trim();
    if !label.is_empty() {
        return label.to_string();
    }
    task.command.chars().take(48).collect()
}

/// 拼出退出确认提示。
///
/// 参数:
/// - `work`: 仍在运行的工作
///
/// 返回:
/// - 给用户看的一行提示
fn format_notice(work: &RunningWork) -> String {
    let mut names = work.subagents.clone();
    names.extend(work.commands.clone());
    let shown = names.iter().take(4).cloned().collect::<Vec<_>>().join("、");
    let extra = names.len().saturating_sub(4);
    let suffix = if extra > 0 {
        format!("，另有 {extra} 项")
    } else {
        String::new()
    };
    if crate::i18n::is_zh() {
        format!(
            "还有 {} 个子智能体、{} 个后台命令在运行（{}{}）。再输入 exit 或再按一次 Ctrl+D 才会关闭，取消则留在会话里。",
            work.subagents.len(),
            work.commands.len(),
            shown,
            suffix
        )
    } else {
        format!(
            "{} subagent(s) and {} background command(s) are still running ({}{}). Type exit or press Ctrl+D again to close; otherwise stay in the session.",
            work.subagents.len(),
            work.commands.len(),
            shown,
            if extra > 0 { format!(", plus {extra} more") } else { String::new() }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::background_exit_notice;
    use crate::tools::subagent_state::{create_subagent_for_owner, finish_subagent};

    /// 关闭当前会话时，不能把另一个并行会话的子智能体算进来。
    #[test]
    fn exit_notice_stays_inside_the_current_session() {
        let temp = tempfile::tempdir().unwrap();
        let paths = crate::paths::SaiPaths::for_tests(temp.path());
        paths.create_dirs().unwrap();
        let current = temp.path().join("session-current");
        let other = temp.path().join("session-other");
        std::fs::create_dir_all(&current).unwrap();
        std::fs::create_dir_all(&other).unwrap();
        let (subagent, _cancel) = create_subagent_for_owner(
            &other.display().to_string(),
            "other session task".to_string(),
            "general".to_string(),
            4,
        );
        assert!(background_exit_notice(&paths, "current", &current).is_none());
        assert!(background_exit_notice(&paths, "other", &other).is_some());
        finish_subagent(&subagent.id, "completed", None, None, None);
    }
}

