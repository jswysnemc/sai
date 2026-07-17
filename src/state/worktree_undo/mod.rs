mod restore;
mod snapshot;

use crate::state::StateStore;
use anyhow::Result;
use std::path::Path;

pub(crate) use restore::restore_latest_snapshot;
use snapshot::{discard_snapshot, finalize_snapshot, start_snapshot, PendingSnapshot};

/// 工作树撤销结果。
#[derive(Debug, Clone)]
pub struct WorktreeUndoOutcome {
    pub restored: bool,
}

/// 单轮工作树快照守卫，正常结束或中断时都会记录运行后的指纹。
pub(crate) struct WorktreeUndoGuard {
    pending: Option<PendingSnapshot>,
}

impl WorktreeUndoGuard {
    /// 在当前工作目录为 Git 仓库时开始记录单轮工作树快照。
    ///
    /// 参数:
    /// - `state`: 当前会话状态
    /// - `workspace`: 当前运行工作目录
    /// - `turn_id`: 当前轮次标识
    ///
    /// 返回:
    /// - 工作树快照守卫
    pub(crate) fn begin(state: &StateStore, workspace: &Path, turn_id: &str) -> Result<Self> {
        Ok(Self {
            pending: start_snapshot(state.state_dir(), workspace, turn_id)?,
        })
    }

    /// 完成快照并保存运行后的工作树指纹。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 完成是否成功
    pub(crate) fn finish(mut self) -> Result<()> {
        if let Some(pending) = self.pending.take() {
            finalize_snapshot(pending)?;
        }
        Ok(())
    }
}

/// 丢弃指定轮次尚未需要撤销的工作树快照。
///
/// 参数:
/// - `state`: 当前会话状态
/// - `turn_id`: 当前轮次标识
///
/// 返回:
/// - 删除是否成功
pub(crate) fn discard_turn_snapshot(state: &StateStore, turn_id: &str) -> Result<()> {
    discard_snapshot(state.state_dir(), turn_id)
}

impl Drop for WorktreeUndoGuard {
    fn drop(&mut self) {
        if let Some(pending) = self.pending.take() {
            let _ = finalize_snapshot(pending);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    /// 执行测试仓库中的 Git 命令。
    fn git(repository: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(repository)
            .status()
            .unwrap();
        assert!(status.success());
    }

    /// 创建包含一个基线文件的 Git 仓库。
    fn repository(root: &Path) {
        git(root, &["init", "--quiet"]);
        git(root, &["config", "user.name", "Sai Test"]);
        git(root, &["config", "user.email", "sai@example.com"]);
        std::fs::write(root.join("tracked.txt"), "base").unwrap();
        git(root, &["add", "tracked.txt"]);
        git(root, &["commit", "--quiet", "-m", "test: baseline"]);
    }

    #[test]
    /// 验证撤销恢复运行前修改并删除本轮新文件。
    fn restore_preserves_pre_turn_changes_and_removes_turn_files() {
        let temp = tempfile::tempdir().unwrap();
        let repository_root = temp.path().join("repository");
        let state_dir = temp.path().join("state");
        std::fs::create_dir_all(&repository_root).unwrap();
        repository(&repository_root);
        std::fs::write(repository_root.join("tracked.txt"), "before").unwrap();
        git(&repository_root, &["add", "tracked.txt"]);
        std::fs::write(repository_root.join("existing.txt"), "before-untracked").unwrap();

        let pending = snapshot::start_snapshot(&state_dir, &repository_root, "turn-1")
            .unwrap()
            .unwrap();
        std::fs::write(repository_root.join("tracked.txt"), "after").unwrap();
        std::fs::write(repository_root.join("existing.txt"), "after-untracked").unwrap();
        std::fs::write(repository_root.join("created.txt"), "created").unwrap();
        snapshot::finalize_snapshot(pending).unwrap();

        let outcome = restore_latest_snapshot(&state_dir, "turn-1").unwrap();

        assert!(outcome.restored);
        assert_eq!(
            std::fs::read_to_string(repository_root.join("tracked.txt")).unwrap(),
            "before"
        );
        assert_eq!(
            std::fs::read_to_string(repository_root.join("existing.txt")).unwrap(),
            "before-untracked"
        );
        assert!(!repository_root.join("created.txt").exists());
        let staged = Command::new("git")
            .args(["diff", "--cached", "--name-only"])
            .current_dir(&repository_root)
            .output()
            .unwrap();
        let unstaged = Command::new("git")
            .args(["diff", "--name-only"])
            .current_dir(&repository_root)
            .output()
            .unwrap();
        assert_eq!(
            String::from_utf8(staged.stdout).unwrap().trim(),
            "tracked.txt"
        );
        assert!(String::from_utf8(unstaged.stdout)
            .unwrap()
            .trim()
            .is_empty());
    }

    #[test]
    /// 验证本轮结束后的新修改会阻止撤销覆盖。
    fn restore_rejects_changes_made_after_snapshot_finalization() {
        let temp = tempfile::tempdir().unwrap();
        let repository_root = temp.path().join("repository");
        let state_dir = temp.path().join("state");
        std::fs::create_dir_all(&repository_root).unwrap();
        repository(&repository_root);

        let pending = snapshot::start_snapshot(&state_dir, &repository_root, "turn-1")
            .unwrap()
            .unwrap();
        std::fs::write(repository_root.join("tracked.txt"), "turn-change").unwrap();
        snapshot::finalize_snapshot(pending).unwrap();
        std::fs::write(repository_root.join("tracked.txt"), "later-change").unwrap();

        let error = restore_latest_snapshot(&state_dir, "turn-1").unwrap_err();

        assert!(error.to_string().contains("changed after the turn"));
        assert_eq!(
            std::fs::read_to_string(repository_root.join("tracked.txt")).unwrap(),
            "later-change"
        );
    }
}
