use anyhow::{bail, Result};
use std::path::Path;

#[path = "git_diff_types.rs"]
mod types;

use types::GitOutput;
pub(crate) use types::{
    GitBranch, GitBranchesResponse, GitCommitDetails, GitCommitDetailsResponse, GitCommitFile,
    GitCommitSummary, GitDiff, GitDiffResponse, GitDirtyCounts, GitFileStatus, GitLogResponse,
    GitOperationResponse, GitRepositoryState, GitStatusEntry,
};

#[path = "git_diff_support.rs"]
mod support;

use support::*;

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
                git_op(root, "stage_all", None, None, None).await?;
            } else {
                for path in paths {
                    git_op(root, "stage", Some(path.as_str()), None, None).await?;
                }
            }
        }
        "unstage" => {
            if paths.is_empty() {
                git_op(root, "unstage_all", None, None, None).await?;
            } else {
                for path in paths {
                    git_op(root, "unstage", Some(path.as_str()), None, None).await?;
                }
            }
        }
        "discard" => {
            if paths.is_empty() {
                bail!("discard requires at least one path");
            }
            for path in paths {
                git_op(root, "discard", Some(path.as_str()), None, None).await?;
            }
        }
        "commit" => {
            git_op(root, "commit", None, message, None).await?;
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
            upstream: String::new(),
            remote_name: String::new(),
            remote_url: String::new(),
            ahead: 0,
            behind: 0,
            stash_count: 0,
            dirty_counts: GitDirtyCounts::default(),
            entries: Vec::new(),
            status: "error".to_string(),
            error: Some(trim_bytes(&output.stderr)),
        });
    }
    let (head, upstream, ahead, behind, stash_count, entries) =
        parse_status_porcelain_v2(&output.stdout);
    let (remote_name, remote_url) = resolve_state_remote(&repo_root, &upstream).await;
    Ok(GitRepositoryState {
        repo_root: repo_root.display().to_string(),
        workdir,
        head,
        upstream,
        remote_name,
        remote_url,
        ahead,
        behind,
        stash_count,
        dirty_counts: dirty_counts(&entries),
        entries,
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
        "--pretty=format:%x1e%H%x1f%h%x1f%P%x1f%D%x1f%an%x1f%ae%x1f%aI%x1f%s".to_string(),
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
            &[
                "rev-list",
                "--left-right",
                "--count",
                &format!("HEAD...{remote_ref}"),
            ],
        )
        .await
        {
            let mut parts = output.stdout.split_whitespace();
            history_ahead = parts
                .next()
                .and_then(|value| value.parse().ok())
                .unwrap_or(0);
            history_behind = parts
                .next()
                .and_then(|value| value.parse().ok())
                .unwrap_or(0);
        }
    }
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = git_success(Path::new(&state.repo_root), &arg_refs).await?;
    let commits = parse_git_log(&output.stdout);
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
    let files: Vec<String> = if let Some(path) = clean_path.clone() {
        vec![path]
    } else {
        state
            .entries
            .iter()
            .map(|entry| entry.path.clone())
            .collect()
    };
    if mode == "working_tree" {
        return working_tree_diff(&state, files, clean_path.as_deref()).await;
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

/// 执行写操作。
pub(crate) async fn git_op(
    root: &Path,
    action: &str,
    path: Option<&str>,
    message: Option<&str>,
    remote_url: Option<&str>,
) -> Result<GitOperationResponse> {
    if action == "init" {
        let branch = message
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("main");
        let result = run_git_output(root, &["init", "-b", branch]).await;
        return operation_response(root, result, "repository initialized").await;
    }

    let state = ensure_ready(root).await?;
    let repo = Path::new(&state.repo_root);
    let result = match action {
        "stage" => {
            let path = validate_repo_relative_path(path.unwrap_or_default())?;
            git_success(repo, &["add", "--", path.as_str()]).await
        }
        "stage_all" => git_success(repo, &["add", "-A", "--"]).await,
        "unstage" => {
            let path = validate_repo_relative_path(path.unwrap_or_default())?;
            if !ref_exists(repo, "HEAD").await {
                git_success(repo, &["rm", "--cached", "--", path.as_str()]).await
            } else {
                git_success(repo, &["restore", "--staged", "--", path.as_str()]).await
            }
        }
        "unstage_all" => {
            if !ref_exists(repo, "HEAD").await {
                if state.dirty_counts.staged > 0 {
                    git_success(repo, &["rm", "--cached", "-r", "--", "."]).await
                } else {
                    Ok(empty_output())
                }
            } else {
                git_success(repo, &["restore", "--staged", "--", "."]).await
            }
        }
        "discard" => {
            let path = validate_repo_relative_path(path.unwrap_or_default())?;
            let is_untracked = state
                .entries
                .iter()
                .any(|entry| entry.path == path && entry.untracked);
            if is_untracked {
                git_success(repo, &["clean", "-fd", "--", path.as_str()]).await
            } else if !ref_exists(repo, "HEAD").await {
                git_success(repo, &["rm", "-f", "--", path.as_str()]).await
            } else {
                git_success(
                    repo,
                    &["restore", "--staged", "--worktree", "--", path.as_str()],
                )
                .await
            }
        }
        "discard_all" => {
            if !ref_exists(repo, "HEAD").await {
                let remove = if state.dirty_counts.staged > 0 {
                    git_success(repo, &["rm", "-f", "-r", "--", "."]).await?
                } else {
                    empty_output()
                };
                let clean = git_success(repo, &["clean", "-fd", "--", "."]).await?;
                Ok(merge_outputs([remove, clean]))
            } else {
                let restore =
                    git_success(repo, &["restore", "--staged", "--worktree", "--", "."]).await?;
                let clean = git_success(repo, &["clean", "-fd", "--", "."]).await?;
                Ok(merge_outputs([restore, clean]))
            }
        }
        "commit" => {
            let message = message
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| anyhow::anyhow!("commit message cannot be empty"))?;
            if state.dirty_counts.staged == 0 {
                bail!("no staged changes to commit");
            }
            git_success(repo, &["commit", "-m", message]).await
        }
        "fetch" => {
            if git_remote_names(repo).await?.is_empty() {
                Err(anyhow::anyhow!("repository has no remote configured"))
            } else {
                git_success(repo, &["fetch", "--prune"]).await
            }
        }
        "pull" => pull_repo(repo, &state).await,
        "push" => push_repo(repo, &state).await,
        "set_remote" => {
            let remote_url = remote_url
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| anyhow::anyhow!("remote URL cannot be empty"))?;
            if git_origin_exists(repo).await {
                git_success(repo, &["remote", "set-url", "origin", remote_url]).await
            } else {
                git_success(repo, &["remote", "add", "origin", remote_url]).await
            }
        }
        "switch_branch" => switch_branch(repo, message).await,
        "create_branch" => {
            let branch = message
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| anyhow::anyhow!("branch name cannot be empty"))?;
            git_success(repo, &["switch", "-c", branch]).await
        }
        "add_to_gitignore" => add_to_gitignore(repo, path).await,
        "stash_push" => {
            let mut args = vec!["stash", "push", "--include-untracked"];
            let owned;
            if let Some(value) = message.map(str::trim).filter(|value| !value.is_empty()) {
                owned = value.to_string();
                args.extend(["-m", owned.as_str()]);
            }
            git_success(repo, &args).await
        }
        "stash_pop" => git_success(repo, &["stash", "pop"]).await,
        _ => bail!("unsupported git action: {action}"),
    };
    let message = match action {
        "stage" | "stage_all" => "files staged",
        "unstage" | "unstage_all" => "files unstaged",
        "discard" | "discard_all" => "changes discarded",
        "commit" => "commit created",
        "fetch" => "fetch completed",
        "pull" => "pull completed",
        "push" => "push completed",
        "set_remote" => "remote repository saved",
        "switch_branch" => "branch switched",
        "create_branch" => "branch created",
        "add_to_gitignore" => "path added to .gitignore",
        "stash_push" => "changes stashed",
        "stash_pop" => "stash popped",
        _ => "operation completed",
    };
    operation_response(root, result, message).await
}
