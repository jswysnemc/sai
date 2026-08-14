use super::frontmatter::{self, Frontmatter};
use super::links::extract_links;
use anyhow::{Context, Result};
use std::path::Path;

/// 一条完整的记忆。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryEntry {
    /// 头部元数据
    pub front: Frontmatter,
    /// 正文
    pub body: String,
}

impl MemoryEntry {
    /// 返回正文里引用的其它记忆标识。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 关联标识
    pub fn links(&self) -> Vec<String> {
        extract_links(&self.body)
    }
}

/// 读取一条记忆。
///
/// 参数:
/// - `path`: 记忆文件路径
///
/// 返回:
/// - 记忆内容；文件不存在或缺少头部时为 None
pub fn read(path: &Path) -> Result<Option<MemoryEntry>> {
    if !path.is_file() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("读取记忆文件失败：{}", path.display()))?;
    let Some((mut front, body)) = frontmatter::split(&content) else {
        return Ok(None);
    };
    // 头部缺 name 时以文件名兜底，否则这条记忆无法被关联引用
    if front.name.trim().is_empty() {
        if let Some(stem) = path.file_stem().and_then(|value| value.to_str()) {
            front.name = stem.to_string();
        }
    }
    Ok(Some(MemoryEntry { front, body }))
}

/// 写入一条记忆。
///
/// 参数:
/// - `path`: 记忆文件路径
/// - `entry`: 记忆内容
///
/// 返回:
/// - 写入结果
pub fn write(path: &Path, entry: &MemoryEntry) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, frontmatter::render(&entry.front, &entry.body))
        .with_context(|| format!("写入记忆文件失败：{}", path.display()))?;
    Ok(())
}

/// 删除一条记忆。
///
/// 参数:
/// - `path`: 记忆文件路径
///
/// 返回:
/// - 文件此前是否存在
pub fn remove(path: &Path) -> Result<bool> {
    if !path.is_file() {
        return Ok(false);
    }
    std::fs::remove_file(path)
        .with_context(|| format!("删除记忆文件失败：{}", path.display()))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::super::memory_type::MemoryType;
    use super::*;

    /// 构造一条测试记忆。
    ///
    /// 参数:
    /// - `name`: 标识
    ///
    /// 返回:
    /// - 记忆内容
    fn entry(name: &str) -> MemoryEntry {
        MemoryEntry {
            front: Frontmatter {
                name: name.to_string(),
                description: "摘要".to_string(),
                memory_type: MemoryType::Feedback,
            },
            body: "正文，关联 [[other]]".to_string(),
        }
    }

    /// 验证写入后能原样读回。
    #[test]
    fn writing_then_reading_returns_the_same_entry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.md");

        write(&path, &entry("a")).unwrap();

        assert_eq!(read(&path).unwrap().unwrap(), entry("a"));
    }

    /// 验证读取不存在的文件返回空而不是报错。
    #[test]
    fn reading_a_missing_file_yields_none() {
        let dir = tempfile::tempdir().unwrap();

        assert!(read(&dir.path().join("missing.md")).unwrap().is_none());
    }

    /// 验证缺少头部的文件被跳过。
    ///
    /// 目录里可能有用户随手放的笔记，把它当记忆读会污染索引。
    #[test]
    fn a_file_without_frontmatter_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plain.md");
        std::fs::write(&path, "只是一段笔记").unwrap();

        assert!(read(&path).unwrap().is_none());
    }

    /// 验证头部缺标识时用文件名兜底。
    ///
    /// 标识为空会让这条记忆无法被任何关联指到。
    #[test]
    fn a_missing_name_falls_back_to_the_file_stem() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("from-filename.md");
        std::fs::write(&path, "---\nname:\ndescription: d\nmetadata:\n  type: user\n---\n正文").unwrap();

        let found = read(&path).unwrap().unwrap();

        assert_eq!(found.front.name, "from-filename");
    }

    /// 验证正文里的关联被解析出来。
    #[test]
    fn links_are_extracted_from_the_body() {
        assert_eq!(entry("a").links(), vec!["other"]);
    }

    /// 验证删除返回文件此前是否存在。
    #[test]
    fn removing_reports_whether_the_file_existed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.md");
        write(&path, &entry("a")).unwrap();

        assert!(remove(&path).unwrap());
        assert!(!remove(&path).unwrap());
    }

    /// 验证写入会自动建立父目录。
    #[test]
    fn writing_creates_missing_parent_directories() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/deep/a.md");

        write(&path, &entry("a")).unwrap();

        assert!(path.is_file());
    }
}
