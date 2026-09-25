use super::limits::DIRECTORY_PAGE_LIMIT;
use crate::tools::fs_path::fs_error;
use anyhow::Result;
use std::path::Path;

/// 分页列出目录项。
///
/// 这是 sai 在 CometixCode Read 之外保留的能力：探索类子智能体只有只读工具，
/// 没有 shell 可以 `ls`。目录项按名称排序，子目录带 `/` 后缀。
///
/// 参数:
/// - `path`: 目录路径
/// - `offset`: 1 起始的起始目录项
/// - `limit`: 最多列出的目录项
///
/// 返回:
/// - 纯文本目录列表
pub(super) fn read_directory(path: &Path, offset: usize, limit: Option<usize>) -> Result<String> {
    let mut entries = Vec::new();
    let reader =
        std::fs::read_dir(path).map_err(|error| fs_error("list directory", path, &error))?;
    for entry in reader {
        let entry = entry.map_err(|error| fs_error("list directory", path, &error))?;
        let is_dir = entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false);
        let name = entry.file_name().to_string_lossy().into_owned();
        entries.push(if is_dir { format!("{name}/") } else { name });
    }
    entries.sort();
    let total = entries.len();
    let limit = limit.unwrap_or(DIRECTORY_PAGE_LIMIT);
    let start = offset.saturating_sub(1).min(total);
    let selected = entries
        .into_iter()
        .skip(start)
        .take(limit)
        .collect::<Vec<_>>();
    let end = start + selected.len();
    let mut output = format!("Directory: {}\n", path.display());
    if total == 0 {
        output.push_str("(empty directory)");
        return Ok(output);
    }
    if selected.is_empty() {
        output.push_str(&format!(
            "(offset {offset} is past the last entry; the directory has {total} entries)"
        ));
        return Ok(output);
    }
    output.push_str(&format!("Entries {}-{end} of {total}:\n", start + 1));
    output.push_str(&selected.join("\n"));
    if end < total {
        output.push_str(&format!(
            "\n(more entries follow; continue with offset={})",
            end + 1
        ));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 目录项排序、子目录带斜杠，并提示续读位置。
    #[test]
    fn lists_sorted_entries_with_pagination_hint() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join("src")).unwrap();
        std::fs::write(temp.path().join("b.txt"), "").unwrap();
        std::fs::write(temp.path().join("a.txt"), "").unwrap();
        let output = read_directory(temp.path(), 1, Some(2)).unwrap();
        assert!(
            output.contains("Entries 1-2 of 3:\na.txt\nb.txt"),
            "{output}"
        );
        assert!(output.contains("continue with offset=3"), "{output}");
        let rest = read_directory(temp.path(), 3, Some(2)).unwrap();
        assert!(rest.contains("src/"), "{rest}");
        assert!(!rest.contains("more entries follow"), "{rest}");
    }
}
