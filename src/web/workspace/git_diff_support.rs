use super::*;
use anyhow::{bail, Result};
use std::path::{Component, Path, PathBuf};

pub(super) async fn pull_repo(repo: &Path, state: &GitRepositoryState) -> Result<GitOutput> {
    if state.upstream.trim().is_empty() {
        if state.head.trim().is_empty() || state.head == "(detached)" {
            Err(anyhow::anyhow!("not on a local branch that can be pulled"))
        } else if !git_origin_exists(repo).await {
            Err(anyhow::anyhow!(
                "current branch has no upstream and origin remote is unavailable"
            ))
        } else {
            git_success(repo, &["pull", "--ff-only", "origin", state.head.as_str()]).await
        }
    } else {
        git_success(repo, &["pull", "--ff-only"]).await
    }
}

pub(super) async fn push_repo(repo: &Path, state: &GitRepositoryState) -> Result<GitOutput> {
    if state.upstream.trim().is_empty() {
        if state.head.trim().is_empty() || state.head == "(detached)" {
            Err(anyhow::anyhow!("not on a local branch that can be pushed"))
        } else if !git_origin_exists(repo).await {
            Err(anyhow::anyhow!(
                "current branch has no upstream and origin remote is unavailable"
            ))
        } else {
            git_success(repo, &["push", "-u", "origin", state.head.as_str()]).await
        }
    } else {
        git_success(repo, &["push"]).await
    }
}

pub(super) async fn add_to_gitignore(repo: &Path, path: Option<&str>) -> Result<GitOutput> {
    let path = validate_repo_relative_path(path.unwrap_or_default())?;
    let pattern = format!("/{path}");
    let gitignore = repo.join(".gitignore");
    let mut content = match tokio::fs::read_to_string(&gitignore).await {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => bail!("failed to read .gitignore: {error}"),
    };
    let already = content.lines().any(|line| {
        let line = line.trim();
        line == path || line == pattern
    });
    if !already {
        if !content.is_empty() && !content.ends_with('\n') {
            content.push('\n');
        }
        content.push_str(&pattern);
        content.push('\n');
        tokio::fs::write(&gitignore, content).await?;
    }
    Ok(empty_output())
}

pub(super) async fn operation_response(
    root: &Path,
    result: Result<GitOutput>,
    message: &str,
) -> Result<GitOperationResponse> {
    match result {
        Ok(output) => Ok(GitOperationResponse {
            ok: true,
            state: git_status(root).await?,
            stdout: output.stdout,
            stderr: output.stderr,
            message: message.to_string(),
        }),
        Err(error) => Ok(GitOperationResponse {
            ok: false,
            state: git_status(root)
                .await
                .unwrap_or_else(|_| not_repo_state(&root.display().to_string())),
            stdout: String::new(),
            stderr: error.to_string(),
            message: error.to_string(),
        }),
    }
}

pub(super) async fn ensure_ready(root: &Path) -> Result<GitRepositoryState> {
    let state = git_status(root).await?;
    if state.status != "ready" {
        bail!(state
            .error
            .unwrap_or_else(|| "current directory is not a Git repository".to_string()));
    }
    Ok(state)
}

pub(super) async fn discover_repo(root: &Path) -> Result<Option<std::path::PathBuf>> {
    let output = git_raw(root, &["rev-parse", "--show-toplevel"]).await?;
    if !output.status.success() {
        return Ok(None);
    }
    let text = trim_bytes(&output.stdout);
    if text.is_empty() {
        return Ok(None);
    }
    Ok(Some(std::path::PathBuf::from(text)))
}

pub(super) fn not_repo_state(workdir: &str) -> GitRepositoryState {
    GitRepositoryState {
        repo_root: String::new(),
        workdir: workdir.to_string(),
        head: String::new(),
        upstream: String::new(),
        remote_name: String::new(),
        remote_url: String::new(),
        ahead: 0,
        behind: 0,
        stash_count: 0,
        dirty_counts: GitDirtyCounts::default(),
        entries: Vec::new(),
        operation: None,
        status: "not_repo".to_string(),
        error: None,
    }
}

/// 检测仓库当前是否处于合并、变基、拣选或还原流程。
///
/// 参数:
/// - `repo_root`: 仓库工作树根目录
///
/// 返回:
/// - 存在进行中操作时返回操作能力，否则返回空
pub(super) async fn detect_in_progress_operation(
    repo_root: &Path,
) -> Option<GitInProgressOperation> {
    let git_dir = resolve_git_dir(repo_root).await?;

    // 1. 变基目录可能采用 merge 或 apply 两种后端
    if path_exists(&git_dir.join("rebase-merge")).await
        || path_exists(&git_dir.join("rebase-apply")).await
    {
        return Some(in_progress_operation("rebase", true));
    }

    // 2. 其余流程通过 Git 写入的状态文件判断
    for (marker, kind, can_skip) in [
        ("MERGE_HEAD", "merge", false),
        ("CHERRY_PICK_HEAD", "cherry_pick", true),
        ("REVERT_HEAD", "revert", true),
    ] {
        if path_exists(&git_dir.join(marker)).await {
            return Some(in_progress_operation(kind, can_skip));
        }
    }
    None
}

/// 解析普通仓库与 worktree 的实际 Git 元数据目录。
///
/// 参数:
/// - `repo_root`: 仓库工作树根目录
///
/// 返回:
/// - 可读取时返回 Git 元数据目录
async fn resolve_git_dir(repo_root: &Path) -> Option<PathBuf> {
    let dot_git = repo_root.join(".git");
    let metadata = tokio::fs::metadata(&dot_git).await.ok()?;
    if metadata.is_dir() {
        return Some(dot_git);
    }
    let content = tokio::fs::read_to_string(&dot_git).await.ok()?;
    let value = content.trim().strip_prefix("gitdir:")?.trim();
    let path = PathBuf::from(value);
    Some(if path.is_absolute() {
        path
    } else {
        repo_root.join(path)
    })
}

/// 判断路径是否存在，读取失败时按不存在处理。
///
/// 参数:
/// - `path`: 待检测路径
///
/// 返回:
/// - 路径是否存在
async fn path_exists(path: &Path) -> bool {
    tokio::fs::try_exists(path).await.unwrap_or(false)
}

/// 创建进行中操作描述。
///
/// 参数:
/// - `kind`: 操作类型
/// - `can_skip`: 是否允许跳过当前提交
///
/// 返回:
/// - 操作能力描述
fn in_progress_operation(kind: &str, can_skip: bool) -> GitInProgressOperation {
    GitInProgressOperation {
        kind: kind.to_string(),
        can_continue: true,
        can_skip,
        can_abort: true,
    }
}

pub(super) fn parse_branch_ab(value: &str) -> (i32, i32) {
    let mut ahead = 0;
    let mut behind = 0;
    for part in value.split_whitespace() {
        if let Some(raw) = part.strip_prefix('+') {
            ahead = raw.parse().unwrap_or(0);
        } else if let Some(raw) = part.strip_prefix('-') {
            behind = raw.parse().unwrap_or(0);
        }
    }
    (ahead, behind)
}

pub(super) fn status_entry(
    path: String,
    old_path: Option<String>,
    index: char,
    worktree: char,
    kind: &str,
) -> GitStatusEntry {
    let conflicted = kind == "conflict" || index == 'U' || worktree == 'U';
    let untracked = kind == "untracked";
    let staged = !untracked && !conflicted && index != '.';
    GitStatusEntry {
        path,
        old_path,
        index_status: index.to_string(),
        worktree_status: worktree.to_string(),
        kind: kind.to_string(),
        staged,
        conflicted,
        untracked,
    }
}

pub(super) fn parse_status_porcelain_v2(
    raw: &[u8],
) -> (String, String, i32, i32, i32, Vec<GitStatusEntry>) {
    let mut head = String::new();
    let mut upstream = String::new();
    let mut ahead = 0;
    let mut behind = 0;
    let mut stash_count = 0;
    let mut entries = Vec::new();
    let records: Vec<String> = raw
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(|part| String::from_utf8_lossy(part).to_string())
        .collect();
    let mut index = 0;
    while index < records.len() {
        let record = records[index].trim_end_matches('\n');
        if let Some(value) = record.strip_prefix("# branch.head ") {
            head = value.trim().to_string();
        } else if let Some(value) = record.strip_prefix("# branch.upstream ") {
            upstream = value.trim().to_string();
        } else if let Some(value) = record.strip_prefix("# branch.ab ") {
            (ahead, behind) = parse_branch_ab(value);
        } else if let Some(value) = record.strip_prefix("# stash ") {
            stash_count = value.trim().parse().unwrap_or(0);
        } else if let Some(rest) = record.strip_prefix("1 ") {
            let fields: Vec<&str> = rest.splitn(8, ' ').collect();
            if fields.len() >= 8 {
                let mut chars = fields[0].chars();
                let ix = chars.next().unwrap_or('.');
                let wt = chars.next().unwrap_or('.');
                entries.push(status_entry(
                    fields[7].to_string(),
                    None,
                    ix,
                    wt,
                    "modified",
                ));
            }
        } else if let Some(rest) = record.strip_prefix("2 ") {
            let fields: Vec<&str> = rest.splitn(9, ' ').collect();
            if fields.len() >= 9 {
                let mut chars = fields[0].chars();
                let ix = chars.next().unwrap_or('.');
                let wt = chars.next().unwrap_or('.');
                let old_path = records.get(index + 1).cloned();
                if old_path.is_some() {
                    index += 1;
                }
                entries.push(status_entry(
                    fields[8].to_string(),
                    old_path,
                    ix,
                    wt,
                    "renamed",
                ));
            }
        } else if let Some(rest) = record.strip_prefix("u ") {
            let fields: Vec<&str> = rest.splitn(10, ' ').collect();
            if fields.len() >= 10 {
                let mut chars = fields[0].chars();
                let ix = chars.next().unwrap_or('U');
                let wt = chars.next().unwrap_or('U');
                entries.push(status_entry(
                    fields[9].to_string(),
                    None,
                    ix,
                    wt,
                    "conflict",
                ));
            }
        } else if let Some(path) = record.strip_prefix("? ") {
            entries.push(status_entry(path.to_string(), None, '?', '?', "untracked"));
        }
        index += 1;
    }
    (head, upstream, ahead, behind, stash_count, entries)
}

pub(super) fn dirty_counts(entries: &[GitStatusEntry]) -> GitDirtyCounts {
    let mut counts = GitDirtyCounts::default();
    for entry in entries {
        if entry.conflicted {
            counts.conflicted += 1;
        } else if entry.untracked {
            counts.untracked += 1;
        } else {
            if entry.index_status != "." {
                counts.staged += 1;
            }
            if entry.worktree_status != "." {
                counts.unstaged += 1;
            }
        }
    }
    counts
}

pub(super) fn parse_git_log(raw: &str) -> Vec<GitCommitSummary> {
    let mut commits = Vec::new();
    for record in raw.split('\x1e') {
        let record = record.trim_matches('\0').trim();
        if record.is_empty() {
            continue;
        }
        let mut parts = record.splitn(2, '\0');
        let header = parts.next().unwrap_or("");
        let files_raw = parts.next().unwrap_or("");
        let fields: Vec<&str> = header.split('\x1f').collect();
        if fields.len() < 8 {
            continue;
        }
        let files = parse_name_status_records(files_raw);
        commits.push(GitCommitSummary {
            sha: fields[0].trim().to_string(),
            short_sha: fields[1].trim().to_string(),
            parents: fields[2].split_whitespace().map(str::to_string).collect(),
            refs: fields[3]
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect(),
            author_name: fields[4].trim().to_string(),
            author_email: fields[5].trim().to_string(),
            author_date: fields[6].trim().to_string(),
            subject: fields[7].trim().to_string(),
            file_count: files.len(),
            files,
            local_only: false,
            remote_only: false,
        });
    }
    commits
}

pub(super) fn parse_name_status_records(raw: &str) -> Vec<GitCommitFile> {
    let mut files = Vec::new();
    let records: Vec<&str> = raw.split('\0').filter(|value| !value.is_empty()).collect();
    let mut index = 0;
    while index < records.len() {
        let status = records[index].trim();
        if status.is_empty() {
            index += 1;
            continue;
        }
        if status.starts_with('R') || status.starts_with('C') {
            let old_path = records.get(index + 1).map(|value| value.to_string());
            let path = records.get(index + 2).unwrap_or(&"").to_string();
            if !path.is_empty() {
                files.push(GitCommitFile {
                    path,
                    old_path,
                    status: status.to_string(),
                    kind: "renamed".to_string(),
                });
            }
            index += 3;
            continue;
        }
        let path = records.get(index + 1).unwrap_or(&"").to_string();
        if !path.is_empty() {
            files.push(GitCommitFile {
                path,
                old_path: None,
                status: status.to_string(),
                kind: "modified".to_string(),
            });
        }
        index += 2;
    }
    files
}

pub(super) fn parse_shortstat(raw: &str) -> (usize, usize, usize) {
    let mut files_changed = 0;
    let mut insertions = 0;
    let mut deletions = 0;
    for part in raw.split(',') {
        let part = part.trim();
        if let Some(value) = part.split_whitespace().next() {
            if part.contains("file") {
                files_changed = value.parse().unwrap_or(0);
            } else if part.contains("insertion") {
                insertions = value.parse().unwrap_or(0);
            } else if part.contains("deletion") {
                deletions = value.parse().unwrap_or(0);
            }
        }
    }
    (files_changed, insertions, deletions)
}

pub(super) fn validate_repo_relative_path(path: &str) -> Result<String> {
    let path = path.trim().replace('\\', "/");
    if path.is_empty() {
        bail!("path cannot be empty");
    }
    if path.starts_with('/') || path.contains('\0') {
        bail!("invalid path");
    }
    let mut normalized = Vec::new();
    for component in Path::new(&path).components() {
        match component {
            Component::Normal(part) => normalized.push(part.to_string_lossy().to_string()),
            Component::CurDir => {}
            _ => bail!("path escapes repository"),
        }
    }
    if normalized.is_empty() {
        bail!("path cannot be empty");
    }
    Ok(normalized.join("/"))
}

pub(super) async fn resolve_state_remote(repo_root: &Path, upstream: &str) -> (String, String) {
    if let Some((remote, _)) = upstream.split_once('/') {
        if let Ok(url) = git_success(repo_root, &["remote", "get-url", remote]).await {
            return (remote.to_string(), url.stdout.trim().to_string());
        }
    }
    if let Ok(url) = git_success(repo_root, &["remote", "get-url", "origin"]).await {
        return ("origin".to_string(), url.stdout.trim().to_string());
    }
    (String::new(), String::new())
}

pub(super) async fn resolve_default_base(repo_root: &Path) -> Option<String> {
    for candidate in ["origin/main", "origin/master", "main", "master"] {
        if ref_exists(repo_root, candidate).await {
            return Some(candidate.to_string());
        }
    }
    None
}

pub(super) async fn ref_exists(repo_root: &Path, reference: &str) -> bool {
    git_success(repo_root, &["rev-parse", "--verify", "--quiet", reference])
        .await
        .is_ok()
}

pub(super) async fn branch_exists_local(repo_root: &Path, branch: &str) -> bool {
    git_success(
        repo_root,
        &[
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ],
    )
    .await
    .is_ok()
}

pub(super) async fn remote_ref_exists(repo_root: &Path, branch: &str) -> bool {
    git_success(
        repo_root,
        &[
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/remotes/{branch}"),
        ],
    )
    .await
    .is_ok()
        || ref_exists(repo_root, branch).await
}

pub(super) async fn git_remote_names(repo_root: &Path) -> Result<Vec<String>> {
    let output = git_success(repo_root, &["remote"]).await?;
    Ok(output
        .stdout
        .lines()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect())
}

pub(super) async fn git_origin_exists(repo_root: &Path) -> bool {
    git_success(repo_root, &["remote", "get-url", "origin"])
        .await
        .is_ok()
}

pub(super) fn empty_output() -> GitOutput {
    GitOutput {
        stdout: String::new(),
        stderr: String::new(),
    }
}

pub(super) fn merge_outputs(outputs: impl IntoIterator<Item = GitOutput>) -> GitOutput {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    for output in outputs {
        if !output.stdout.trim().is_empty() {
            stdout.push(output.stdout);
        }
        if !output.stderr.trim().is_empty() {
            stderr.push(output.stderr);
        }
    }
    GitOutput {
        stdout: stdout.join("\n"),
        stderr: stderr.join("\n"),
    }
}

pub(super) fn empty_git_diff() -> GitDiff {
    GitDiff {
        repository: false,
        branch: String::new(),
        status: String::new(),
        files: Vec::new(),
        diff: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_branch_ab() {
        assert_eq!(parse_branch_ab("+2 -1"), (2, 1));
    }

    #[test]
    fn parses_status_records() {
        let raw = b"# branch.head main\0# branch.upstream origin/main\0# branch.ab +1 -0\01 M. N... 100644 100644 100644 111 222 333 src/main.rs\0? notes.md\0";
        let (head, upstream, ahead, behind, _, files) = parse_status_porcelain_v2(raw);
        assert_eq!(head, "main");
        assert_eq!(upstream, "origin/main");
        assert_eq!(ahead, 1);
        assert_eq!(behind, 0);
        assert_eq!(files.len(), 2);
        assert!(files[1].untracked);
    }
}
