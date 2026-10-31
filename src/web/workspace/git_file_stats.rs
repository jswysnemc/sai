//! 一次读取工作树各文件增删统计，兼容重命名、二进制和未跟踪文件。

use super::diff_content::read_untracked_text;
use super::{ensure_ready, git_success, git_success_with_input, ref_exists};
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GitFileStats {
    pub(crate) added: u64,
    pub(crate) removed: u64,
    pub(crate) binary: bool,
}

/// 【终端审查】【文件统计】一次查询所有已跟踪文件，再补充小型未跟踪文本的行数。
/// 参数: root 为工作目录；返回: 仓库相对路径与统计，不修改工作树或暂存区。
pub(crate) async fn git_file_stats(root: &Path) -> Result<BTreeMap<String, GitFileStats>> {
    let state = ensure_ready(root).await?;
    let repo = Path::new(&state.repo_root);
    let base = if ref_exists(repo, "HEAD").await {
        "HEAD".to_string()
    } else {
        git_success_with_input(repo, &["hash-object", "-t", "tree", "--stdin"], b"")
            .await?
            .stdout
            .trim()
            .to_string()
    };
    let output = git_success(
        repo,
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--numstat",
            "-z",
            "--find-renames",
            &base,
            "--",
        ],
    )
    .await?;
    let mut stats = parse(&output.stdout)?;
    for entry in state.entries {
        if entry.untracked {
            let text = read_untracked_text(repo, &entry.path).await?;
            stats.insert(
                entry.path,
                text.map(|text| GitFileStats {
                    added: text.split_inclusive('\n').count() as u64,
                    ..Default::default()
                })
                .unwrap_or(GitFileStats {
                    binary: true,
                    ..Default::default()
                }),
            );
        } else {
            stats.entry(entry.path).or_default();
        }
    }
    Ok(stats)
}

/// 【终端审查】【统计解析】按 NUL 分隔记录读取路径，重命名使用目标路径。
/// 参数: output 为 numstat 输出；返回: 每个文件的增删行数。
fn parse(output: &str) -> Result<BTreeMap<String, GitFileStats>> {
    let mut result = BTreeMap::new();
    let mut records = output.split('\0').filter(|value| !value.is_empty());
    while let Some(record) = records.next() {
        let mut fields = record.splitn(3, '\t');
        let added = fields.next().context("missing added count")?;
        let removed = fields.next().context("missing removed count")?;
        let path = fields.next().context("missing file path")?;
        let path = if path.is_empty() {
            records.next().context("missing rename source")?;
            records.next().context("missing rename target")?
        } else {
            path
        };
        let binary = added == "-" || removed == "-";
        let stats = if binary {
            GitFileStats {
                binary,
                ..Default::default()
            }
        } else {
            GitFileStats {
                added: added.parse()?,
                removed: removed.parse()?,
                binary,
            }
        };
        result.insert(path.to_string(), stats);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    /// 【终端审查】【特殊路径测试】重命名、制表符和二进制不影响统计对应关系。
    /// 参数: 无；返回: 无。
    #[test]
    fn parses_renamed_binary_and_tabbed_paths() {
        let stats = parse("2\t1\t\0old\0new\0-\t-\timage.bin\0").unwrap();
        assert_eq!(stats["new"].added, 2);
        assert_eq!(stats["new"].removed, 1);
        assert!(stats["image.bin"].binary);
        assert_eq!(parse("3\t0\twith\ttab\0").unwrap()["with\ttab"].added, 3);
    }
}
