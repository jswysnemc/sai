use anyhow::{bail, Result};
use std::collections::HashSet;
use std::path::Path;

#[path = "git_diff_types.rs"]
mod types;

use types::GitOutput;
pub(crate) use types::{
    GitBranch, GitBranchesResponse, GitCommitDetails, GitCommitDetailsResponse, GitCommitFile,
    GitCommitSummary, GitConflictContent, GitDiff, GitDiffResponse, GitDirtyCounts, GitFileStatus,
    GitInProgressOperation, GitLogResponse, GitOperationRequest, GitOperationResponse, GitRemote,
    GitRepositoriesResponse, GitRepositoryResources, GitRepositoryState,
    GitRepositoryStatusesResponse, GitRepositorySummary, GitStashEntry, GitStatusEntry, GitTag,
    GitWorktree,
};

#[path = "git_diff_support.rs"]
mod support;

#[path = "git_diff_content.rs"]
mod diff_content;

#[path = "git_file_compare.rs"]
mod file_compare;

#[path = "git_process.rs"]
mod process;

#[path = "git_branches.rs"]
mod branches;

#[path = "git_operations.rs"]
mod operations;

#[path = "git_history_operations.rs"]
mod history_operations;

#[path = "git_resources.rs"]
mod resources;

#[path = "git_resource_operations.rs"]
mod resource_operations;

#[path = "git_conflicts.rs"]
mod conflicts;

#[path = "git_repositories.rs"]
mod repositories;

#[path = "git_repository_statuses.rs"]
mod repository_statuses;

#[path = "git_clone.rs"]
mod clone;

#[path = "git_publish.rs"]
mod publish;

#[path = "git_worktrees.rs"]
mod worktrees;

#[path = "git_watcher.rs"]
mod watcher;

use branches::*;
pub(crate) use clone::git_clone;
pub(crate) use conflicts::git_conflict;
use conflicts::resolve_conflict;
use diff_content::*;
pub(crate) use file_compare::git_file_compare;
use history_operations::*;
pub(crate) use operations::git_op;
use process::*;
use publish::*;
pub(crate) use repositories::{
    git_repositories_with_options, validate_git_repository_root, validate_git_repository_roots,
    GitRepositoryDiscoveryOptions,
};
pub(crate) use repository_statuses::git_repository_statuses;
use resource_operations::*;
pub(crate) use resources::git_resources;
use support::*;
pub(crate) use watcher::{GitWatchEvent, RepositoryWatcher};
use worktrees::{add_worktree, git_worktrees, remove_worktree};

#[cfg(test)]
#[path = "git_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "git_conflict_tests.rs"]
mod conflict_tests;

#[cfg(test)]
#[path = "git_repository_tests.rs"]
mod repository_tests;

#[cfg(test)]
#[path = "git_clone_tests.rs"]
mod clone_tests;

#[cfg(test)]
#[path = "git_publish_tests.rs"]
mod publish_tests;

#[cfg(test)]
#[path = "git_file_compare_tests.rs"]
mod file_compare_tests;

const GIT_DIFF_MAX_BYTES: usize = 512 * 1024;
const GIT_LOG_DEFAULT_LIMIT: usize = 50;
const GIT_LOG_MAX_LIMIT: usize = 200;

/// 读取旧版兼容 Diff。
pub(crate) async fn read_git_diff(root: &Path) -> Result<GitDiff> {
    let state = git_status(root).await?;
    if state.status != "ready" {
        return Ok(empty_git_diff());
    }
    let patch = match git_diff(root, "working_tree", None).await {
        Ok(value) => value.patch,
        Err(_) => String::new(),
    };
    Ok(GitDiff {
        repository: true,
        branch: state.head.clone(),
        status: format!("## {}", state.head),
        files: state
            .entries
            .iter()
            .map(|entry| GitFileStatus {
                path: entry.path.clone(),
                index_status: entry.index_status.replace('.', " "),
                worktree_status: entry.worktree_status.replace('.', " "),
            })
            .collect(),
        diff: patch,
    })
}

/// 执行旧版兼容 Git 操作。
pub(crate) async fn apply_git_action(
    root: &Path,
    action: &str,
    paths: &[String],
    message: Option<&str>,
) -> Result<GitDiff> {
    match action {
        "init" => {
            let branch = message
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("main");
            run_git(root, &["init", "-b", branch]).await?;
        }
        "stage" => {
            if paths.is_empty() {
                git_op(root, GitOperationRequest::new("stage_all")).await?;
            } else {
                for path in paths {
                    git_op(root, GitOperationRequest::new("stage").with_path(path)).await?;
                }
            }
        }
        "unstage" => {
            if paths.is_empty() {
                git_op(root, GitOperationRequest::new("unstage_all")).await?;
            } else {
                for path in paths {
                    git_op(root, GitOperationRequest::new("unstage").with_path(path)).await?;
                }
            }
        }
        "discard" => {
            if paths.is_empty() {
                bail!("discard requires at least one path");
            }
            for path in paths {
                git_op(root, GitOperationRequest::new("discard").with_path(path)).await?;
            }
        }
        "commit" => {
            git_op(
                root,
                GitOperationRequest::new("commit").with_message(message),
            )
            .await?;
        }
        _ => bail!("unsupported git action: {action}"),
    }
    read_git_diff(root).await
}

/// 读取仓库状态。
pub(crate) async fn git_status(root: &Path) -> Result<GitRepositoryState> {
    let workdir = root.display().to_string();
    let Some(repo_root) = discover_repo(root).await? else {
        return Ok(not_repo_state(&workdir));
    };
    let output = git_raw(
        &repo_root,
        &["status", "--porcelain=v2", "--branch", "--show-stash", "-z"],
    )
    .await?;
    if !output.status.success() {
        return Ok(GitRepositoryState {
            repo_root: repo_root.display().to_string(),
            workdir,
            head: String::new(),
            has_commits: false,
            upstream: String::new(),
            remote_name: String::new(),
            remote_url: String::new(),
            ahead: 0,
            behind: 0,
            stash_count: 0,
            dirty_counts: GitDirtyCounts::default(),
            entries: Vec::new(),
            operation: None,
            status: "error".to_string(),
            error: Some(trim_bytes(&output.stderr)),
        });
    }
    let (head, has_commits, upstream, ahead, behind, stash_count, entries) =
        parse_status_porcelain_v2(&output.stdout);
    let (remote_name, remote_url) = resolve_state_remote(&repo_root, &upstream).await;
    let operation = detect_in_progress_operation(&repo_root).await;
    Ok(GitRepositoryState {
        repo_root: repo_root.display().to_string(),
        workdir,
        head,
        has_commits,
        upstream,
        remote_name,
        remote_url,
        ahead,
        behind,
        stash_count,
        dirty_counts: dirty_counts(&entries),
        entries,
        operation,
        status: "ready".to_string(),
        error: None,
    })
}

/// 读取分支列表。
pub(crate) async fn git_branches(root: &Path) -> Result<GitBranchesResponse> {
    let state = git_status(root).await?;
    if state.status != "ready" {
        return Ok(GitBranchesResponse {
            state,
            branches: Vec::new(),
        });
    }
    let mut branches = Vec::new();
    let local = git_success(
        Path::new(&state.repo_root),
        &[
            "for-each-ref",
            "--format=%(refname:short)%00%(upstream:short)%00%(HEAD)",
            "refs/heads",
        ],
    )
    .await?;
    for line in local.stdout.split('\n') {
        if line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split('\0').collect();
        if parts.is_empty() || parts[0].is_empty() {
            continue;
        }
        let name = parts[0].to_string();
        let upstream = parts.get(1).unwrap_or(&"").to_string();
        let current = parts.get(2).map(|value| *value == "*").unwrap_or(false);
        let (ahead, behind) = if current {
            (state.ahead, state.behind)
        } else {
            (0, 0)
        };
        branches.push(GitBranch {
            name: name.clone(),
            full_name: name,
            kind: "local".to_string(),
            current,
            upstream,
            ahead,
            behind,
        });
    }
    let remote = git_success(
        Path::new(&state.repo_root),
        &["for-each-ref", "--format=%(refname:short)", "refs/remotes"],
    )
    .await
    .unwrap_or_else(|_| empty_output());
    for name in remote.stdout.lines() {
        let name = name.trim();
        if name.is_empty() || name.ends_with("/HEAD") {
            continue;
        }
        branches.push(GitBranch {
            name: name.to_string(),
            full_name: name.to_string(),
            kind: "remote".to_string(),
            current: false,
            upstream: String::new(),
            ahead: 0,
            behind: 0,
        });
    }
    if !state.head.is_empty()
        && state.head != "(detached)"
        && !branches
            .iter()
            .any(|branch| branch.kind == "local" && branch.full_name == state.head)
    {
        branches.insert(
            0,
            GitBranch {
                name: state.head.clone(),
                full_name: state.head.clone(),
                kind: "local".to_string(),
                current: true,
                upstream: state.upstream.clone(),
                ahead: state.ahead,
                behind: state.behind,
            },
        );
    }
    Ok(GitBranchesResponse { state, branches })
}

/// 读取提交历史。
pub(crate) async fn git_log(
    root: &Path,
    limit: Option<usize>,
    skip: Option<usize>,
) -> Result<GitLogResponse> {
    let state = git_status(root).await?;
    if state.status != "ready" || !ref_exists(Path::new(&state.repo_root), "HEAD").await {
        return Ok(GitLogResponse {
            state,
            commits: Vec::new(),
            history_base_ref: String::new(),
            history_remote_ref: String::new(),
            history_ahead: 0,
            history_behind: 0,
            merge_base: String::new(),
        });
    }
    let limit = limit
        .unwrap_or(GIT_LOG_DEFAULT_LIMIT)
        .clamp(1, GIT_LOG_MAX_LIMIT);
    let skip = skip.unwrap_or(0);
    let mut args = vec![
        "log".to_string(),
        "--date=iso-strict".to_string(),
        "--decorate=short".to_string(),
        "--topo-order".to_string(),
        "--parents".to_string(),
        "--name-status".to_string(),
        "-z".to_string(),
        "--find-renames".to_string(),
        format!("--max-count={limit}"),
        "--pretty=format:%x1e%H%x1f%h%x1f%P%x1f%D%x1f%an%x1f%ae%x1f%aI%x1f%s%x00".to_string(),
    ];
    if skip > 0 {
        args.push(format!("--skip={skip}"));
    }
    let remote_ref = if state.upstream.trim().is_empty() {
        String::new()
    } else {
        state.upstream.clone()
    };
    let mut history_ahead = state.ahead;
    let mut history_behind = state.behind;
    let mut merge_base = String::new();
    let mut local_only = HashSet::new();
    let mut remote_only = HashSet::new();
    if !remote_ref.is_empty() {
        if let Ok(output) = git_success(
            Path::new(&state.repo_root),
            &["merge-base", "HEAD", remote_ref.as_str()],
        )
        .await
        {
            merge_base = output.stdout.trim().to_string();
        }
        if let Ok(output) = git_success(
            Path::new(&state.repo_root),
            &["rev-list", "--left-right", &format!("HEAD...{remote_ref}")],
        )
        .await
        {
            for line in output.stdout.lines() {
                if let Some(sha) = line.strip_prefix('<') {
                    local_only.insert(sha.to_string());
                } else if let Some(sha) = line.strip_prefix('>') {
                    remote_only.insert(sha.to_string());
                }
            }
            history_ahead = local_only.len() as i32;
            history_behind = remote_only.len() as i32;
        }
    }
    args.push("HEAD".to_string());
    if !remote_ref.is_empty() {
        args.push(remote_ref.clone());
    }
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = git_success(Path::new(&state.repo_root), &arg_refs).await?;
    let mut commits = parse_git_log(&output.stdout);
    for commit in &mut commits {
        commit.local_only = local_only.contains(&commit.sha);
        commit.remote_only = remote_only.contains(&commit.sha);
    }
    Ok(GitLogResponse {
        state,
        commits,
        history_base_ref: merge_base.clone(),
        history_remote_ref: remote_ref,
        history_ahead,
        history_behind,
        merge_base,
    })
}

/// 读取提交详情。
pub(crate) async fn git_commit_details(
    root: &Path,
    commit: &str,
) -> Result<GitCommitDetailsResponse> {
    let state = ensure_ready(root).await?;
    let commit = commit.trim();
    if commit.is_empty() {
        bail!("commit sha cannot be empty");
    }
    let metadata = git_success(
        Path::new(&state.repo_root),
        &[
            "show",
            "-s",
            "--date=iso-strict",
            "--format=%H%x1f%h%x1f%an%x1f%ae%x1f%aI%x1f%s%x1f%b",
            commit,
        ],
    )
    .await?;
    let fields: Vec<&str> = metadata.stdout.splitn(7, '\x1f').collect();
    if fields.len() < 7 {
        bail!("unable to parse commit details");
    }
    let files_output = git_success(
        Path::new(&state.repo_root),
        &[
            "show",
            "--format=",
            "--name-status",
            "-z",
            "--find-renames",
            commit,
        ],
    )
    .await?;
    let files = parse_name_status_records(&files_output.stdout);
    let stat = git_success(
        Path::new(&state.repo_root),
        &["show", "--format=", "--stat", "--find-renames", commit],
    )
    .await?;
    let shortstat = git_success(
        Path::new(&state.repo_root),
        &["show", "--format=", "--shortstat", "--find-renames", commit],
    )
    .await?;
    let (files_changed, insertions, deletions) = parse_shortstat(&shortstat.stdout);
    Ok(GitCommitDetailsResponse {
        commit: GitCommitDetails {
            sha: fields[0].trim().to_string(),
            short_sha: fields[1].trim().to_string(),
            author_name: fields[2].trim().to_string(),
            author_email: fields[3].trim().to_string(),
            author_date: fields[4].trim().to_string(),
            subject: fields[5].trim().to_string(),
            body: fields[6].trim().to_string(),
            file_count: files.len(),
            files,
            files_changed,
            insertions,
            deletions,
            stat: stat.stdout.trim().to_string(),
            remote_name: state.remote_name.clone(),
            remote_url: state.remote_url.clone(),
        },
        state,
    })
}

/// 读取工作树/分支 Diff。
pub(crate) async fn git_diff(
    root: &Path,
    mode: &str,
    path: Option<&str>,
) -> Result<GitDiffResponse> {
    let state = ensure_ready(root).await?;
    let clean_path = path.map(validate_repo_relative_path).transpose()?;
    let files = diff_files(&state, mode, clean_path.as_deref())?;
    match mode {
        "working_tree" => {
            return working_tree_diff(&state, files, clean_path.as_deref()).await;
        }
        "unstaged" => {
            return unstaged_diff(&state, files, clean_path.as_deref()).await;
        }
        "staged" => {
            return staged_diff(&state, files, clean_path.as_deref()).await;
        }
        "branch" => {}
        _ => bail!("unsupported git diff mode: {mode}"),
    }

    let base_ref = if !state.upstream.trim().is_empty() {
        state.upstream.clone()
    } else {
        resolve_default_base(Path::new(&state.repo_root))
            .await
            .unwrap_or_default()
    };
    if base_ref.is_empty() {
        bail!("unable to find a base branch for review; set an upstream or fetch the main branch first");
    }
    let mut args = vec![
        "diff".to_string(),
        "--patch".to_string(),
        "--stat".to_string(),
        format!("{base_ref}...HEAD"),
    ];
    if let Some(path) = clean_path.as_deref() {
        args.push("--".to_string());
        args.push(path.to_string());
    }
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = git_success(Path::new(&state.repo_root), &arg_refs).await?;
    let (stat, patch) = split_stat_and_patch(&output.stdout);
    let (patch, truncated) = truncate_patch(patch);
    Ok(GitDiffResponse {
        base_ref,
        head_ref: "HEAD".to_string(),
        mode: mode.to_string(),
        files,
        patch,
        stat,
        truncated,
        binary_files: Vec::new(),
    })
}

/// 读取提交 Diff。
pub(crate) async fn git_commit_diff(
    root: &Path,
    commit: &str,
    path: Option<&str>,
) -> Result<GitDiffResponse> {
    let state = ensure_ready(root).await?;
    let commit = commit.trim();
    if commit.is_empty() {
        bail!("commit sha cannot be empty");
    }
    let clean_path = path.map(validate_repo_relative_path).transpose()?;
    let parent_output = git_success(
        Path::new(&state.repo_root),
        &["show", "-s", "--format=%P", commit],
    )
    .await?;
    let first_parent = parent_output
        .stdout
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_string();
    let mut args: Vec<String> = if first_parent.is_empty() {
        vec![
            "show".to_string(),
            "--format=".to_string(),
            "--patch".to_string(),
            "--stat".to_string(),
            "--find-renames".to_string(),
            commit.to_string(),
        ]
    } else {
        vec![
            "diff".to_string(),
            "--patch".to_string(),
            "--stat".to_string(),
            "--find-renames".to_string(),
            first_parent.clone(),
            commit.to_string(),
        ]
    };
    if let Some(path) = clean_path.as_deref() {
        args.push("--".to_string());
        args.push(path.to_string());
    }
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = git_success(Path::new(&state.repo_root), &arg_refs).await?;
    let (stat, patch) = split_stat_and_patch(&output.stdout);
    let (patch, truncated) = truncate_patch(patch);
    Ok(GitDiffResponse {
        base_ref: if first_parent.is_empty() {
            "ROOT".to_string()
        } else {
            first_parent
        },
        head_ref: commit.to_string(),
        mode: "commit".to_string(),
        files: clean_path.into_iter().collect(),
        patch,
        stat,
        truncated,
        binary_files: Vec::new(),
    })
}
