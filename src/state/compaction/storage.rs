use super::model::CompactionSummary;
use anyhow::{Context, Result};
use chrono::Utc;
use std::path::Path;

/// 读取压缩摘要。
///
/// 参数:
/// - `path`: 摘要文件路径
///
/// 返回:
/// - 压缩摘要，文件不存在时返回空
pub fn load_summary(path: &Path) -> Result<Option<CompactionSummary>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let summary = serde_json::from_str::<CompactionSummary>(&raw)
        .with_context(|| format!("invalid JSON in {}", path.display()))?;
    Ok(Some(summary))
}

/// 保存压缩摘要。
///
/// 参数:
/// - `path`: 摘要文件路径
/// - `summary`: 摘要正文
/// - `compacted_turns`: 已压缩总轮次数
///
/// 返回:
/// - 保存是否成功
pub fn save_summary(path: &Path, summary: &str, compacted_turns: usize) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let value = CompactionSummary {
        updated_at: Utc::now().to_rfc3339(),
        compacted_turns,
        summary: summary.trim().to_string(),
    };
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    use std::io::Write;
    temporary.write_all(format!("{}\n", serde_json::to_string_pretty(&value)?).as_bytes())?;
    temporary.persist(path)?;
    Ok(())
}

/// 清理压缩摘要。
///
/// 参数:
/// - `path`: 摘要文件路径
///
/// 返回:
/// - 清理是否成功
pub fn clear_summary(path: &Path) -> Result<()> {
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_and_loads_summary() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("summary.json");

        save_summary(&path, "summary", 3).unwrap();
        let summary = load_summary(&path).unwrap().unwrap();

        assert_eq!(summary.summary, "summary");
        assert_eq!(summary.compacted_turns, 3);
    }

    #[test]
    fn missing_summary_is_empty() {
        let temp = tempfile::tempdir().unwrap();

        assert!(load_summary(&temp.path().join("missing.json"))
            .unwrap()
            .is_none());
    }
}
