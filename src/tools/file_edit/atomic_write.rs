use crate::tools::fs_path::fs_error;
use anyhow::Result;
use std::path::Path;

/// 原子写入 UTF-8 文本。
///
/// 先写同目录临时文件再 persist，避免写入中断留下半截文件。
/// 每一步失败都附带目标路径，模型才能区分是目录不存在、权限不足还是磁盘问题。
///
/// 参数:
/// - `path`: 目标文件路径
/// - `content`: 待写入文本
///
/// 返回:
/// - 写入是否成功
pub(crate) fn write_text_file(path: &Path, content: &str) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    // 1. 补齐缺失的上级目录
    std::fs::create_dir_all(parent)
        .map_err(|error| fs_error("create parent directory", parent, &error))?;
    // 2. 同目录临时文件承载完整内容，保证 persist 是原子替换
    let temp = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| fs_error("create temporary file in", parent, &error))?;
    std::fs::write(temp.path(), content.as_bytes())
        .map_err(|error| fs_error("write file", path, &error))?;
    // 3. 原子替换目标文件
    temp.persist(path)
        .map_err(|error| fs_error("write file", path, &error.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证写入时自动创建缺失的上级目录。
    #[test]
    fn creates_missing_parent_directories() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("nested/deeper/sample.txt");

        write_text_file(&path, "content").unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "content");
    }

    /// 验证覆盖写入替换原有内容。
    #[test]
    fn overwrites_existing_content() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("sample.txt");
        std::fs::write(&path, "old").unwrap();

        write_text_file(&path, "new").unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
    }
}
