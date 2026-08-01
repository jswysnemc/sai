use anyhow::Result;
use std::path::{Path, PathBuf};

/// 每个会话保留的已完成快照数量上限。
///
/// 撤销只会回到最近若干轮，更早的快照留着没有用途，却会持续占用磁盘。
pub(super) const MAX_RETAINED_SNAPSHOTS: usize = 5;

/// 清理结果统计。
#[derive(Debug, Default, Eq, PartialEq)]
pub(crate) struct CleanupReport {
    /// 清理掉的孤儿 pending 目录数量
    pub(crate) orphaned: usize,
    /// 按保留上限淘汰的已完成快照数量
    pub(crate) evicted: usize,
}

impl CleanupReport {
    /// 判断本次清理是否有实际删除。
    ///
    /// 返回:
    /// - 存在任一类删除时返回 true
    pub(crate) fn is_empty(&self) -> bool {
        self.orphaned == 0 && self.evicted == 0
    }
}

/// 清理快照目录中的残留与超量快照。
///
/// 进程被强制结束时 `finish` 与 `Drop` 都不会执行，`pending_` 目录会永久
/// 留在磁盘上；已完成快照同样没有数量上限。两者叠加会让状态目录膨胀到
/// 数 GB。会话启动时调用本函数收敛。
///
/// 参数:
/// - `snapshot_root`: 会话的 worktree-undo 根目录
///
/// 返回:
/// - 清理统计；目录不存在时返回空统计
pub(crate) fn cleanup_snapshot_root(snapshot_root: &Path) -> Result<CleanupReport> {
    let mut report = CleanupReport::default();
    if !snapshot_root.is_dir() {
        return Ok(report);
    }
    let mut completed = Vec::new();
    // 1. 分拣目录：pending_ 前缀为未收尾残留，turn_ 前缀为已完成快照
    for entry in std::fs::read_dir(snapshot_root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("pending_") {
            std::fs::remove_dir_all(&path)?;
            report.orphaned += 1;
            continue;
        }
        if name.starts_with("turn_") {
            completed.push((modified_at(&path), path));
        }
    }
    // 2. 已完成快照按修改时间保留最近若干个，其余淘汰
    completed.sort_by(|left, right| right.0.cmp(&left.0));
    for (_, path) in completed.into_iter().skip(MAX_RETAINED_SNAPSHOTS) {
        std::fs::remove_dir_all(&path)?;
        report.evicted += 1;
    }
    Ok(report)
}

/// 读取目录修改时间，失败时视为最早。
///
/// 参数:
/// - `path`: 目标目录
///
/// 返回:
/// - 修改时间；无法读取时返回 UNIX 纪元
fn modified_at(path: &PathBuf) -> std::time::SystemTime {
    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .unwrap_or(std::time::UNIX_EPOCH)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建一个含占位文件的快照目录。
    fn make_snapshot(root: &Path, name: &str) -> PathBuf {
        let path = root.join(name);
        std::fs::create_dir_all(path.join("untracked")).unwrap();
        std::fs::write(path.join("record.json"), "{}").unwrap();
        path
    }

    /// 目录不存在时返回空统计而不报错。
    #[test]
    fn missing_root_reports_nothing() {
        let temp = tempfile::tempdir().unwrap();

        let report = cleanup_snapshot_root(&temp.path().join("absent")).unwrap();

        assert!(report.is_empty());
    }

    /// 崩溃残留的 pending 目录应当被删除。
    #[test]
    fn removes_orphaned_pending_directories() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let pending = make_snapshot(root, "pending_abc");
        let kept = make_snapshot(root, "turn_abc");

        let report = cleanup_snapshot_root(root).unwrap();

        assert_eq!(report.orphaned, 1);
        assert!(!pending.exists());
        assert!(kept.exists(), "已完成快照不受影响");
    }

    /// 超过保留上限的已完成快照应当被淘汰。
    #[test]
    fn evicts_snapshots_beyond_the_retention_limit() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        for index in 0..(MAX_RETAINED_SNAPSHOTS + 3) {
            make_snapshot(root, &format!("turn_{index:02}"));
        }

        let report = cleanup_snapshot_root(root).unwrap();

        assert_eq!(report.evicted, 3);
        let remaining = std::fs::read_dir(root).unwrap().count();
        assert_eq!(remaining, MAX_RETAINED_SNAPSHOTS);
    }

    /// 未超过上限时不淘汰任何快照。
    #[test]
    fn keeps_every_snapshot_within_the_limit() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        for index in 0..MAX_RETAINED_SNAPSHOTS {
            make_snapshot(root, &format!("turn_{index:02}"));
        }

        let report = cleanup_snapshot_root(root).unwrap();

        assert!(report.is_empty());
        assert_eq!(
            std::fs::read_dir(root).unwrap().count(),
            MAX_RETAINED_SNAPSHOTS
        );
    }

    /// 无关目录与文件不受影响。
    #[test]
    fn leaves_unrelated_entries_untouched() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        std::fs::create_dir_all(root.join("other")).unwrap();
        std::fs::write(root.join("note.txt"), "x").unwrap();

        let report = cleanup_snapshot_root(root).unwrap();

        assert!(report.is_empty());
        assert!(root.join("other").exists());
        assert!(root.join("note.txt").exists());
    }
}
