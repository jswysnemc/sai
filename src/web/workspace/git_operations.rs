use super::*;
use anyhow::{bail, Result};
use std::path::Path;

/// 执行会修改仓库的 Git 操作。
///
/// 参数:
/// - `root`: 当前工作区目录
/// - `request`: 操作名称与可选参数
///
/// 返回:
/// - 操作结果、Git 输出与刷新后的仓库状态
pub(crate) async fn git_op(
    root: &Path,
    request: GitOperationRequest<'_>,
) -> Result<GitOperationResponse> {
    if request.action == "init" {
        let branch = request
            .message
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("main");
        let result = run_git_output(root, &["init", "-b", branch]).await;
        return operation_response(root, result, "repository initialized").await;
    }

    let state = ensure_ready(root).await?;
    let repo = Path::new(&state.repo_root);
    let result = dispatch_operation(repo, &state, &request).await;
    operation_response(root, result, operation_message(request.action)).await
}

/// 按操作名称分派真实 Git CLI 命令。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `state`: 执行前仓库状态
/// - `request`: 操作参数
///
/// 返回:
/// - Git 命令输出
async fn dispatch_operation(
    repo: &Path,
    state: &GitRepositoryState,
    request: &GitOperationRequest<'_>,
) -> Result<GitOutput> {
    match request.action {
        "stage" => stage_path(repo, request.path).await,
        "stage_all" => git_success(repo, &["add", "-A", "--"]).await,
        "unstage" => unstage_path(repo, request.path).await,
        "unstage_all" => unstage_all(repo, state).await,
        "discard" => discard_path(repo, state, request.path, request.old_path).await,
        "discard_all" => discard_all(repo, state).await,
        "stage_patch" => apply_patch(repo, request.patch, PatchOperation::Stage).await,
        "unstage_patch" => apply_patch(repo, request.patch, PatchOperation::Unstage).await,
        "discard_patch" => apply_patch(repo, request.patch, PatchOperation::Discard).await,
        "commit" => commit(repo, state, request).await,
        "fetch" => fetch(repo).await,
        "pull" => pull_repo(repo, state).await,
        "pull_rebase" => pull_rebase(repo, state).await,
        "push" => push_repo(repo, state).await,
        "force_push_with_lease" => force_push_with_lease(repo, state).await,
        "sync" => sync_repo(repo, state).await,
        "set_remote" => set_origin_remote(repo, request.remote_url).await,
        "publish" => publish_repository(repo, state, request.remote_url).await,
        "switch_branch" => {
            switch_branch(
                repo,
                request.branch.or(request.message),
                request.branch_kind,
            )
            .await
        }
        "create_branch" => {
            create_branch(
                repo,
                request.branch.or(request.message),
                request.start_point,
            )
            .await
        }
        "rename_branch" => rename_branch(repo, request.branch, request.new_branch).await,
        "delete_branch" => delete_branch(repo, state, request.branch, request.force).await,
        "merge_branch" => merge_branch(repo, state, request.branch).await,
        "rebase_branch" => rebase_branch(repo, state, request.branch).await,
        "checkout_commit" => checkout_commit(repo, request.commit).await,
        "cherry_pick" => cherry_pick_commit(repo, request.commit).await,
        "rebase_onto" => rebase_onto_commit(repo, request.commit).await,
        "reset_commit" => reset_commit(repo, request.commit, request.reset_mode).await,
        "revert_commit" => revert_commit(repo, request.commit).await,
        "add_to_gitignore" => add_to_gitignore(repo, request.path).await,
        "stash_push" => stash_push(repo, request.message, request.include_untracked).await,
        "stash_apply" => stash_apply(repo, request.stash_ref).await,
        "stash_pop" => stash_pop(repo, request.stash_ref).await,
        "stash_drop" => stash_drop(repo, request.stash_ref).await,
        "tag_create" => create_tag(repo, request.tag, request.commit).await,
        "tag_delete" => delete_tag(repo, request.tag).await,
        "remote_add" => add_remote(repo, request.remote_name, request.remote_url).await,
        "remote_remove" => remove_remote(repo, request.remote_name).await,
        "worktree_add" => {
            add_worktree(
                repo,
                request.workspace_root.map(Path::new).unwrap_or(repo),
                request.worktree_path,
                request.branch,
                request.new_branch,
            )
            .await
        }
        "worktree_remove" => remove_worktree(repo, request.worktree_path, request.force).await,
        "resolve_conflict" => {
            resolve_conflict(
                repo,
                state,
                request.path,
                request.resolution,
                request.content,
            )
            .await
        }
        "continue_operation" => continue_operation(repo, state).await,
        "skip_operation" => skip_operation(repo, state).await,
        "abort_operation" => abort_operation(repo, state).await,
        _ => bail!("unsupported git action: {}", request.action),
    }
}

/// 暂存单个仓库相对路径。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `path`: 仓库相对路径
///
/// 返回:
/// - Git 命令输出
async fn stage_path(repo: &Path, path: Option<&str>) -> Result<GitOutput> {
    let path = validate_repo_relative_path(path.unwrap_or_default())?;
    git_success(repo, &["add", "--", path.as_str()]).await
}

/// 取消暂存单个路径，并兼容尚无 HEAD 的仓库。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `path`: 仓库相对路径
///
/// 返回:
/// - Git 命令输出
async fn unstage_path(repo: &Path, path: Option<&str>) -> Result<GitOutput> {
    let path = validate_repo_relative_path(path.unwrap_or_default())?;
    if ref_exists(repo, "HEAD").await {
        git_success(repo, &["restore", "--staged", "--", path.as_str()]).await
    } else {
        git_success(repo, &["rm", "--cached", "--", path.as_str()]).await
    }
}

/// 取消暂存仓库中的全部路径。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `state`: 当前仓库状态
///
/// 返回:
/// - Git 命令输出
async fn unstage_all(repo: &Path, state: &GitRepositoryState) -> Result<GitOutput> {
    if ref_exists(repo, "HEAD").await {
        git_success(repo, &["restore", "--staged", "--", "."]).await
    } else if state.dirty_counts.staged > 0 {
        git_success(repo, &["rm", "--cached", "-r", "--", "."]).await
    } else {
        Ok(empty_output())
    }
}

/// 丢弃单个文件的工作树和暂存区修改。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `state`: 当前仓库状态
/// - `path`: 当前路径
/// - `old_path`: 重命名前路径
///
/// 返回:
/// - Git 命令输出
async fn discard_path(
    repo: &Path,
    state: &GitRepositoryState,
    path: Option<&str>,
    old_path: Option<&str>,
) -> Result<GitOutput> {
    let path = validate_repo_relative_path(path.unwrap_or_default())?;
    let old_path = old_path
        .filter(|value| !value.trim().is_empty())
        .map(validate_repo_relative_path)
        .transpose()?;
    let is_untracked = state
        .entries
        .iter()
        .any(|entry| entry.path == path && entry.untracked);
    if is_untracked {
        return git_success(repo, &["clean", "-fd", "--", path.as_str()]).await;
    }
    if !ref_exists(repo, "HEAD").await {
        return git_success(repo, &["rm", "-f", "--", path.as_str()]).await;
    }
    let mut args = vec!["restore", "--staged", "--worktree", "--", path.as_str()];
    if let Some(old_path) = old_path.as_deref().filter(|old_path| *old_path != path) {
        args.push(old_path);
    }
    git_success(repo, &args).await
}

/// 丢弃仓库中的全部已跟踪和未跟踪修改。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `state`: 当前仓库状态
///
/// 返回:
/// - 合并后的 Git 命令输出
async fn discard_all(repo: &Path, state: &GitRepositoryState) -> Result<GitOutput> {
    let tracked = if ref_exists(repo, "HEAD").await {
        git_success(repo, &["restore", "--staged", "--worktree", "--", "."]).await?
    } else if state.dirty_counts.staged > 0 {
        git_success(repo, &["rm", "-f", "-r", "--", "."]).await?
    } else {
        empty_output()
    };
    let untracked = git_success(repo, &["clean", "-fd", "--", "."]).await?;
    Ok(merge_outputs([tracked, untracked]))
}

#[derive(Clone, Copy)]
enum PatchOperation {
    Stage,
    Unstage,
    Discard,
}

/// 将统一 Diff 应用到暂存区或工作树，实现部分暂存、取消暂存和丢弃。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `patch`: 前端选择的 unified patch
/// - `operation`: patch 应用目标与方向
///
/// 返回:
/// - Git 命令输出
async fn apply_patch(
    repo: &Path,
    patch: Option<&str>,
    operation: PatchOperation,
) -> Result<GitOutput> {
    let patch = validate_patch(patch)?;
    let args: &[&str] = match operation {
        PatchOperation::Stage => &["apply", "--cached", "--recount", "--whitespace=nowarn", "-"],
        PatchOperation::Unstage => &[
            "apply",
            "--cached",
            "--reverse",
            "--recount",
            "--whitespace=nowarn",
            "-",
        ],
        PatchOperation::Discard => &[
            "apply",
            "--reverse",
            "--recount",
            "--whitespace=nowarn",
            "-",
        ],
    };
    git_success_with_input(repo, args, patch.as_bytes()).await
}

/// 校验前端传入的部分 Diff，限制大小并拒绝非 unified patch 文本。
///
/// 参数:
/// - `patch`: 待校验 patch
///
/// 返回:
/// - 校验通过的原始 patch
fn validate_patch(patch: Option<&str>) -> Result<&str> {
    let patch = patch
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("patch cannot be empty"))?;
    if patch.len() > GIT_DIFF_MAX_BYTES {
        bail!("patch exceeds the maximum supported size");
    }
    if !patch.contains("diff --git ") || !patch.contains("@@") {
        bail!("patch must be a unified git diff");
    }
    Ok(patch)
}

/// 创建提交，并按请求选择全部暂存、修订、签署和后续同步动作。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `state`: 当前仓库状态
/// - `request`: 提交参数
///
/// 返回:
/// - 提交与后续动作的合并输出
async fn commit(
    repo: &Path,
    state: &GitRepositoryState,
    request: &GitOperationRequest<'_>,
) -> Result<GitOutput> {
    let message = request
        .message
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("commit message cannot be empty"))?;
    if state.dirty_counts.conflicted > 0 {
        bail!("resolve all merge conflicts before committing");
    }

    // 1. Commit All 按配置暂存全部文件或仅暂存已经跟踪的文件
    let staged = if request.all {
        let args: &[&str] = if request.exclude_untracked {
            &["add", "-u", "--"]
        } else {
            &["add", "-A", "--"]
        };
        Some(git_success(repo, args).await?)
    } else {
        None
    };
    if !request.all && !request.amend && !request.allow_empty && state.dirty_counts.staged == 0 {
        bail!("no staged changes to commit");
    }

    // 2. 组合提交参数，所有行为仍由系统 Git 校验
    let mut args = vec!["commit", "-m", message];
    if request.amend {
        args.push("--amend");
    }
    if request.signoff {
        args.push("--signoff");
    }
    if request.allow_empty {
        args.push("--allow-empty");
    }
    let committed = git_success(repo, &args).await?;
    let mut outputs = Vec::new();
    if let Some(staged) = staged {
        outputs.push(staged);
    }
    outputs.push(committed);

    // 3. 可选后续动作失败时保留真实失败结果和已完成提交状态
    match request.post_action.unwrap_or("") {
        "" => {}
        "push" => outputs.push(push_repo(repo, &git_status(repo).await?).await?),
        "sync" => outputs.push(sync_repo(repo, &git_status(repo).await?).await?),
        value => bail!("unsupported post-commit action: {value}"),
    }
    Ok(merge_outputs(outputs))
}

/// 获取全部远端引用并清理失效引用。
///
/// 参数:
/// - `repo`: 仓库根目录
///
/// 返回:
/// - Git 命令输出
async fn fetch(repo: &Path) -> Result<GitOutput> {
    if git_remote_names(repo).await?.is_empty() {
        bail!("repository has no remote configured");
    }
    git_success(repo, &["fetch", "--prune"]).await
}

/// 使用变基方式获取当前分支上游提交。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `state`: 当前仓库状态
///
/// 返回:
/// - Git 命令输出
async fn pull_rebase(repo: &Path, state: &GitRepositoryState) -> Result<GitOutput> {
    if state.upstream.trim().is_empty() {
        if state.head.trim().is_empty() || state.head == "(detached)" {
            bail!("not on a local branch that can be pulled");
        }
        if !git_origin_exists(repo).await {
            bail!("current branch has no upstream and origin remote is unavailable");
        }
        git_success(repo, &["pull", "--rebase", "origin", state.head.as_str()]).await
    } else {
        git_success(repo, &["pull", "--rebase"]).await
    }
}

/// 使用 force-with-lease 推送当前分支。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `state`: 当前仓库状态
///
/// 返回:
/// - Git 命令输出
async fn force_push_with_lease(repo: &Path, state: &GitRepositoryState) -> Result<GitOutput> {
    if state.upstream.trim().is_empty() {
        if state.head.trim().is_empty() || state.head == "(detached)" {
            bail!("not on a local branch that can be pushed");
        }
        if !git_origin_exists(repo).await {
            bail!("current branch has no upstream and origin remote is unavailable");
        }
        git_success(
            repo,
            &[
                "push",
                "--force-with-lease",
                "-u",
                "origin",
                state.head.as_str(),
            ],
        )
        .await
    } else {
        git_success(repo, &["push", "--force-with-lease"]).await
    }
}

/// 依次执行 pull 与 push，任一步失败即返回失败。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `state`: 当前仓库状态
///
/// 返回:
/// - 两次命令的合并输出
async fn sync_repo(repo: &Path, state: &GitRepositoryState) -> Result<GitOutput> {
    let pulled = pull_repo(repo, state).await?;
    let refreshed = git_status(repo).await?;
    let pushed = push_repo(repo, &refreshed).await?;
    Ok(merge_outputs([pulled, pushed]))
}

/// 继续当前合并、变基、拣选或还原流程。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `state`: 当前仓库状态
///
/// 返回:
/// - Git 命令输出
async fn continue_operation(repo: &Path, state: &GitRepositoryState) -> Result<GitOutput> {
    let operation = require_operation(state)?;
    match operation.kind.as_str() {
        "merge" => git_success(repo, &["commit", "--no-edit"]).await,
        "rebase" => git_success(repo, &["rebase", "--continue"]).await,
        "cherry_pick" => git_success(repo, &["cherry-pick", "--continue"]).await,
        "revert" => git_success(repo, &["revert", "--continue"]).await,
        value => bail!("unsupported in-progress operation: {value}"),
    }
}

/// 跳过当前变基、拣选或还原提交。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `state`: 当前仓库状态
///
/// 返回:
/// - Git 命令输出
async fn skip_operation(repo: &Path, state: &GitRepositoryState) -> Result<GitOutput> {
    let operation = require_operation(state)?;
    if !operation.can_skip {
        bail!("{} operation cannot skip the current step", operation.kind);
    }
    match operation.kind.as_str() {
        "rebase" => git_success(repo, &["rebase", "--skip"]).await,
        "cherry_pick" => git_success(repo, &["cherry-pick", "--skip"]).await,
        "revert" => git_success(repo, &["revert", "--skip"]).await,
        value => bail!("unsupported in-progress operation: {value}"),
    }
}

/// 中止当前合并、变基、拣选或还原流程。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `state`: 当前仓库状态
///
/// 返回:
/// - Git 命令输出
async fn abort_operation(repo: &Path, state: &GitRepositoryState) -> Result<GitOutput> {
    let operation = require_operation(state)?;
    match operation.kind.as_str() {
        "merge" => git_success(repo, &["merge", "--abort"]).await,
        "rebase" => git_success(repo, &["rebase", "--abort"]).await,
        "cherry_pick" => git_success(repo, &["cherry-pick", "--abort"]).await,
        "revert" => git_success(repo, &["revert", "--abort"]).await,
        value => bail!("unsupported in-progress operation: {value}"),
    }
}

/// 取得当前进行中操作，不存在时返回可读错误。
///
/// 参数:
/// - `state`: 当前仓库状态
///
/// 返回:
/// - 进行中操作引用
fn require_operation(state: &GitRepositoryState) -> Result<&GitInProgressOperation> {
    state
        .operation
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("repository has no operation in progress"))
}

/// 返回操作成功提示文本。
///
/// 参数:
/// - `action`: 操作名称
///
/// 返回:
/// - 成功提示
fn operation_message(action: &str) -> &'static str {
    match action {
        "stage" | "stage_all" | "stage_patch" => "files staged",
        "unstage" | "unstage_all" | "unstage_patch" => "files unstaged",
        "discard" | "discard_all" | "discard_patch" => "changes discarded",
        "commit" => "commit created",
        "fetch" => "fetch completed",
        "pull" | "pull_rebase" => "pull completed",
        "push" | "force_push_with_lease" => "push completed",
        "sync" => "sync completed",
        "set_remote" => "remote repository saved",
        "publish" => "repository published",
        "switch_branch" => "branch switched",
        "create_branch" => "branch created",
        "rename_branch" => "branch renamed",
        "delete_branch" => "branch deleted",
        "merge_branch" => "branch merged",
        "rebase_branch" => "branch rebased",
        "checkout_commit" => "commit checked out",
        "cherry_pick" => "commit cherry-picked",
        "rebase_onto" => "branch rebased",
        "reset_commit" => "branch reset",
        "revert_commit" => "commit reverted",
        "add_to_gitignore" => "path added to .gitignore",
        "stash_push" => "changes stashed",
        "stash_apply" => "stash applied",
        "stash_pop" => "stash popped",
        "stash_drop" => "stash dropped",
        "tag_create" => "tag created",
        "tag_delete" => "tag deleted",
        "remote_add" => "remote added",
        "remote_remove" => "remote removed",
        "worktree_add" => "worktree created",
        "worktree_remove" => "worktree removed",
        "resolve_conflict" => "conflict resolved",
        "continue_operation" => "operation continued",
        "skip_operation" => "operation step skipped",
        "abort_operation" => "operation aborted",
        _ => "operation completed",
    }
}
