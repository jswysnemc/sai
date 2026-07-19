use super::*;
use anyhow::Result;
use std::collections::BTreeMap;
use std::path::Path;

/// 读取仓库的 stash、标签和远端列表。
///
/// 参数:
/// - `root`: 当前工作区目录
///
/// 返回:
/// - 仓库状态与资源列表
pub(crate) async fn git_resources(root: &Path) -> Result<GitRepositoryResources> {
    let state = ensure_ready(root).await?;
    let repo = Path::new(&state.repo_root);
    let (stashes, tags, remotes) =
        tokio::join!(read_stashes(repo), read_tags(repo), read_remotes(repo));
    Ok(GitRepositoryResources {
        state,
        stashes: stashes?,
        tags: tags?,
        remotes: remotes?,
    })
}

/// 读取 stash 列表。
///
/// 参数:
/// - `repo`: 仓库根目录
///
/// 返回:
/// - 按 Git 默认顺序排列的 stash
async fn read_stashes(repo: &Path) -> Result<Vec<GitStashEntry>> {
    let output = git_success(
        repo,
        &[
            "stash",
            "list",
            "--date=iso-strict",
            "--format=%gd%x1f%H%x1f%gs%x1f%aI",
        ],
    )
    .await?;
    Ok(output
        .stdout
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.splitn(4, '\x1f').collect();
            (fields.len() == 4).then(|| GitStashEntry {
                reference: fields[0].trim().to_string(),
                sha: fields[1].trim().to_string(),
                subject: fields[2].trim().to_string(),
                created_at: fields[3].trim().to_string(),
            })
        })
        .collect())
}

/// 读取标签列表及目标提交信息。
///
/// 参数:
/// - `repo`: 仓库根目录
///
/// 返回:
/// - 按创建时间倒序排列的标签
async fn read_tags(repo: &Path) -> Result<Vec<GitTag>> {
    let output = git_success(
        repo,
        &[
            "for-each-ref",
            "--sort=-creatordate",
            "--format=%(refname:short)%1f%(*objectname)%1f%(objectname)%1f%(creatordate:iso-strict)%1f%(subject)",
            "refs/tags",
        ],
    )
    .await?;
    Ok(output
        .stdout
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.splitn(5, '\x1f').collect();
            if fields.len() != 5 {
                return None;
            }
            let peeled = fields[1].trim();
            Some(GitTag {
                name: fields[0].trim().to_string(),
                sha: if peeled.is_empty() { fields[2] } else { peeled }
                    .trim()
                    .to_string(),
                created_at: fields[3].trim().to_string(),
                subject: fields[4].trim().to_string(),
            })
        })
        .collect())
}

/// 读取远端 fetch 与 push 地址。
///
/// 参数:
/// - `repo`: 仓库根目录
///
/// 返回:
/// - 按远端名称排序的地址列表
async fn read_remotes(repo: &Path) -> Result<Vec<GitRemote>> {
    let output = git_success(repo, &["remote", "-v"]).await?;
    let mut remotes: BTreeMap<String, GitRemote> = BTreeMap::new();
    for line in output.stdout.lines() {
        let mut fields = line.split_whitespace();
        let Some(name) = fields.next() else { continue };
        let Some(url) = fields.next() else { continue };
        let kind = fields.next().unwrap_or("");
        let entry = remotes
            .entry(name.to_string())
            .or_insert_with(|| GitRemote {
                name: name.to_string(),
                fetch_url: String::new(),
                push_url: String::new(),
            });
        if kind == "(push)" {
            entry.push_url = url.to_string();
        } else {
            entry.fetch_url = url.to_string();
        }
    }
    Ok(remotes.into_values().collect())
}
