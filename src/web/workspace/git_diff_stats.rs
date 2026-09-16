use super::diff_content::read_untracked_text;
use super::{ensure_ready, git_success, git_success_with_input, ref_exists};
use anyhow::{Context, Result};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct GitDiffStats {
    pub added: u64,
    pub removed: u64,
}

/// 【工作概览】【改动统计】统计 HEAD 到工作树的增删行数，不构造或传输补丁正文。
/// 参数：`root` 为工作区或仓库目录；返回包含未跟踪小型文本的增删行数。
pub(crate) async fn git_diff_stats(root: &Path) -> Result<GitDiffStats> {
    let state = ensure_ready(root).await?;
    let repo = Path::new(&state.repo_root);
    // 1. 【工作概览】【改动统计】未建立首次提交时使用空树，保持暂存与未暂存改动合并后的语义
    let base = if ref_exists(repo, "HEAD").await {
        "HEAD".to_string()
    } else {
        git_success_with_input(repo, &["hash-object", "-t", "tree", "--stdin"], b"")
            .await?
            .stdout
    };
    let output = git_success(
        repo,
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--numstat",
            "-z",
            &base,
            "--",
        ],
    )
    .await?;
    let mut stats = parse_numstat(&output.stdout)?;
    // 2. 【工作概览】【改动统计】复用补丁接口对未跟踪文件的大小与文本限制
    for entry in state.entries.iter().filter(|entry| entry.untracked) {
        if let Ok(Some(text)) = read_untracked_text(repo, &entry.path).await {
            stats.added = stats
                .added
                .saturating_add(text.split_inclusive('\n').count() as u64);
        }
    }
    Ok(stats)
}

/// 【工作概览】【统计解析】读取以 NUL 分隔的 numstat，兼容重命名路径和二进制文件。
/// 参数：`output` 为 Git 原始统计文本；返回累计的增删行数。
fn parse_numstat(output: &str) -> Result<GitDiffStats> {
    let mut stats = GitDiffStats::default();
    let mut records = output.split('\0').filter(|record| !record.is_empty());
    while let Some(record) = records.next() {
        let mut fields = record.splitn(3, '\t');
        let added = fields.next().context("missing added line count")?;
        let removed = fields.next().context("missing removed line count")?;
        let path = fields.next().context("missing numstat path")?;
        if path.is_empty() {
            // 1. 【工作概览】【统计解析】重命名记录在统计头之后单独提供旧路径与新路径
            records.next().context("missing rename source")?;
            records.next().context("missing rename target")?;
        }
        if added == "-" || removed == "-" {
            continue;
        }
        stats.added = stats
            .added
            .saturating_add(added.parse::<u64>().context("invalid added line count")?);
        stats.removed = stats.removed.saturating_add(
            removed
                .parse::<u64>()
                .context("invalid removed line count")?,
        );
    }
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【工作概览】【解析测试】重命名与特殊文件名不影响数值累加；无参数，无返回值。
    #[test]
    fn parses_renames_binary_entries_and_special_paths() {
        let output = "2\t1\t\0old\tname\0new\nname\0-\t-\timage.bin\05\t3\tfile\tname\0";
        assert_eq!(
            parse_numstat(output).unwrap(),
            GitDiffStats {
                added: 7,
                removed: 4
            }
        );
    }
}
