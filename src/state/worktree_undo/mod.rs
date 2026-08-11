mod limits;
mod restore;
mod retention;
mod snapshot;

use crate::state::StateStore;
use anyhow::Result;
use std::path::Path;

pub(crate) use restore::{restore_latest_snapshot, restore_snapshot_paths};
pub(crate) use retention::cleanup_snapshot_root;
pub(crate) use snapshot::snapshot_root;
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
    /// 快照属于回复完成后的辅助能力，固化失败只影响撤销入口，不能把已经完整
    /// 返回的模型回复改判为失败。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    pub(crate) fn finish(mut self) {
        if let Some(pending) = self.pending.take() {
            if let Err(error) = finalize_snapshot(pending) {
                eprintln!("【会话】【工作树快照】固化失败，已保留本轮回复: {error:#}");
            }
        }
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
    /// 超过大小上限的未跟踪文件不进入快照，避免构建产物撑爆状态目录。
    fn large_untracked_files_stay_out_of_the_snapshot() {
        let temp = tempfile::tempdir().unwrap();
        let repository_root = temp.path().join("repository");
        let state_dir = temp.path().join("state");
        std::fs::create_dir_all(&repository_root).unwrap();
        repository(&repository_root);
        // 一个小文件与一个超过上限的大文件同时未跟踪
        std::fs::write(repository_root.join("small.txt"), "keep me").unwrap();
        std::fs::write(
            repository_root.join("huge.bin"),
            vec![0u8; (limits::MAX_UNTRACKED_FILE_BYTES + 1) as usize],
        )
        .unwrap();

        let pending = snapshot::start_snapshot(&state_dir, &repository_root, "turn-big")
            .unwrap()
            .unwrap();
        snapshot::finalize_snapshot(pending).unwrap();

        // 小文件进入快照，大文件只留登记不留内容
        let snapshot_dir = snapshot::snapshot_directory(&state_dir, "turn-big");
        let copied = walk_files(&snapshot_dir.join("untracked"));
        let names = copied
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().to_string())
            .collect::<Vec<_>>();
        assert!(names.contains(&"small.txt".to_string()));
        assert!(
            !names.contains(&"huge.bin".to_string()),
            "大文件不应被复制进快照: {names:?}"
        );
    }

    /// 会话读取期间的快照清理不能删除仍在运行的轮次快照。
    ///
    /// Web 在流式响应期间会重新打开会话读取时间线。重新打开会触发快照清理；
    /// 如果清理直接删除刚创建的 pending 目录，模型回复完成后的固化操作就会返回
    /// `No such file or directory (os error 2)`，并把成功响应错误标记为本轮失败。
    #[test]
    fn session_reopen_cleanup_keeps_running_snapshot() {
        let temp = tempfile::tempdir().unwrap();
        let repository_root = temp.path().join("repository");
        let state_dir = temp.path().join("state");
        std::fs::create_dir_all(&repository_root).unwrap();
        repository(&repository_root);

        // 1. 模拟模型轮次开始时创建运行中快照
        let pending = snapshot::start_snapshot(&state_dir, &repository_root, "turn-running")
            .unwrap()
            .unwrap();

        // 2. 模拟 Web 刷新时间线时重新打开会话并执行残留清理
        let root = snapshot::snapshot_root(&state_dir);
        let report = cleanup_snapshot_root(&root).unwrap();

        // 3. 运行中快照必须保留，并能在回复完成后正常固化
        assert_eq!(report.orphaned, 0);
        snapshot::finalize_snapshot(pending).unwrap();
        assert!(snapshot::snapshot_directory(&state_dir, "turn-running").is_dir());
    }

    /// 递归收集目录下的全部文件路径。
    fn walk_files(root: &Path) -> Vec<std::path::PathBuf> {
        let mut files = Vec::new();
        let Ok(entries) = std::fs::read_dir(root) else {
            return files;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(walk_files(&path));
            } else {
                files.push(path);
            }
        }
        files
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
