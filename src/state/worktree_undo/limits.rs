use serde::{Deserialize, Serialize};

/// 单个未跟踪文件纳入快照的大小上限（16 MiB）。
///
/// 撤销快照的价值在于恢复模型改动过的源码。构建产物、源码压缩包、
/// 数据集等大文件既不该由模型编辑，复制它们又会让状态目录迅速膨胀到
/// 数 GB。超过该上限的文件只登记元信息，不复制内容。
pub(super) const MAX_UNTRACKED_FILE_BYTES: u64 = 16 * 1024 * 1024;

/// 因超过大小上限而未纳入快照的未跟踪文件。
#[derive(Debug, Deserialize, Serialize)]
pub(super) struct SkippedEntry {
    /// 相对仓库根目录的路径
    pub(super) path: String,
    /// 跳过时的文件字节数
    pub(super) bytes: u64,
}

/// 判断未跟踪文件是否因体积过大而跳过。
///
/// 参数:
/// - `bytes`: 文件字节数
///
/// 返回:
/// - 超过上限时返回 true
pub(super) fn exceeds_snapshot_limit(bytes: u64) -> bool {
    bytes > MAX_UNTRACKED_FILE_BYTES
}

/// 生成跳过文件的用户可读说明。
///
/// 参数:
/// - `skipped`: 本次快照跳过的文件
///
/// 返回:
/// - 说明文本；没有跳过项时返回空
#[cfg(test)]
pub(super) fn describe_skipped(skipped: &[SkippedEntry]) -> String {
    if skipped.is_empty() {
        return String::new();
    }
    let total: u64 = skipped.iter().map(|entry| entry.bytes).sum();
    let mut lines = vec![format!(
        "以下 {} 个未跟踪文件超过 {} MiB，未纳入撤销快照（共 {:.1} MiB）：",
        skipped.len(),
        MAX_UNTRACKED_FILE_BYTES / 1024 / 1024,
        total as f64 / 1024.0 / 1024.0
    )];
    for entry in skipped.iter().take(10) {
        lines.push(format!(
            "  {} ({:.1} MiB)",
            entry.path,
            entry.bytes as f64 / 1024.0 / 1024.0
        ));
    }
    if skipped.len() > 10 {
        lines.push(format!("  ... 另有 {} 个文件", skipped.len() - 10));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 上限以内的文件应当纳入快照。
    #[test]
    fn keeps_files_within_the_limit() {
        assert!(!exceeds_snapshot_limit(0));
        assert!(!exceeds_snapshot_limit(MAX_UNTRACKED_FILE_BYTES));
    }

    /// 超过上限的文件应当跳过。
    #[test]
    fn skips_files_above_the_limit() {
        assert!(exceeds_snapshot_limit(MAX_UNTRACKED_FILE_BYTES + 1));
    }

    /// 没有跳过项时不产生说明文本。
    #[test]
    fn describes_nothing_without_skipped_entries() {
        assert!(describe_skipped(&[]).is_empty());
    }

    /// 跳过项说明包含数量、路径与体积。
    #[test]
    fn describes_skipped_entries_with_paths_and_size() {
        let skipped = vec![SkippedEntry {
            path: "dist/bundle.tar.gz".to_string(),
            bytes: 32 * 1024 * 1024,
        }];

        let text = describe_skipped(&skipped);

        assert!(text.contains("dist/bundle.tar.gz"));
        assert!(text.contains("32.0 MiB"));
        assert!(text.contains('1'));
    }

    /// 超过十项时只列前十并给出剩余数量。
    #[test]
    fn truncates_long_skipped_lists() {
        let skipped = (0..12)
            .map(|index| SkippedEntry {
                path: format!("build/artifact-{index}.bin"),
                bytes: 20 * 1024 * 1024,
            })
            .collect::<Vec<_>>();

        let text = describe_skipped(&skipped);

        assert!(text.contains("build/artifact-0.bin"));
        assert!(text.contains("build/artifact-9.bin"));
        assert!(!text.contains("build/artifact-10.bin"));
        assert!(text.contains("另有 2 个文件"));
    }
}
