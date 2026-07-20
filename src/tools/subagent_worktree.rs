//! Sub-agent git worktree isolation and auto-merge into the parent checkout.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const CREATE_WORKTREE_MAX_ATTEMPTS: usize = 8;
const WORKTREE_DIR_MARKER: &str = ".sai-subagents";
const BRANCH_PREFIX: &str = "sai/subagent/";

/// Isolated worktree created for one sub-agent run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SubagentWorktree {
    pub(crate) repo_root: PathBuf,
    pub(crate) parent_workdir: PathBuf,
    pub(crate) worktree_root: PathBuf,
    pub(crate) workdir: PathBuf,
    pub(crate) branch_name: String,
}

/// Result of applying worktree changes back into the parent checkout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SubagentWorktreeApplyResult {
    pub(crate) applied: bool,
    pub(crate) changed: bool,
    pub(crate) status: String,
    pub(crate) patch_bytes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) skipped_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) apply_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) fallback_reason: Option<String>,
    pub(crate) copied_files: Vec<String>,
    pub(crate) deleted_files: Vec<String>,
    pub(crate) conflict_files: Vec<String>,
}

/// Cleanup outcome for a delegated worktree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SubagentWorktreeCleanupResult {
    pub(crate) removed: bool,
    pub(crate) branch_deleted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) skipped_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}

/// Try to create an isolated worktree for `parent_workdir`.
///
/// Returns `Ok(None)` when the directory is not a git checkout or is already
/// inside a Sai sub-agent worktree.
pub(crate) fn try_create(parent_workdir: &Path, label: &str) -> Result<Option<SubagentWorktree>> {
    let parent_workdir = match fs::canonicalize(parent_workdir) {
        Ok(path) => path,
        Err(_) => return Ok(None),
    };
    if is_sai_subagent_worktree(&parent_workdir) {
        return Ok(None);
    }
    let Ok(repo_root_raw) = run_git(&parent_workdir, &["rev-parse", "--show-toplevel"]) else {
        return Ok(None);
    };
    let repo_root = canonicalize_existing_dir(&repo_root_raw, "git repo root")?;
    let relative_workdir = parent_workdir
        .strip_prefix(&repo_root)
        .map_err(|_| anyhow::anyhow!("workdir must be inside the git repository root"))?;

    let repo_name = repo_root
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| sanitize_path_component(name, "repo"))
        .unwrap_or_else(|| "repo".to_string());
    let label = sanitize_path_component(label, "agent");
    let target_parent = repo_root
        .parent()
        .unwrap_or_else(|| repo_root.as_path())
        .join(WORKTREE_DIR_MARKER)
        .join(&repo_name);
    fs::create_dir_all(&target_parent).context("failed to create worktree parent")?;

    let mut last_error: Option<String> = None;
    let mut created: Option<(PathBuf, String)> = None;
    for _ in 0..CREATE_WORKTREE_MAX_ATTEMPTS {
        let suffix = unique_worktree_suffix();
        let target = target_parent.join(format!("{label}-{suffix}"));
        let branch_name = format!("{BRANCH_PREFIX}{label}-{suffix}");
        match run_git_owned(
            &repo_root,
            vec![
                "worktree".to_string(),
                "add".to_string(),
                "-b".to_string(),
                branch_name.clone(),
                display_path(&target),
                "HEAD".to_string(),
            ],
        ) {
            Ok(_) => {
                created = Some((target, branch_name));
                break;
            }
            Err(err) if is_worktree_name_collision(&err) => {
                last_error = Some(err);
            }
            Err(err) => bail!("failed to create subagent worktree: {err}"),
        }
    }
    let (target, branch_name) = created.ok_or_else(|| {
        anyhow::anyhow!(
            "failed to create a unique subagent worktree after {CREATE_WORKTREE_MAX_ATTEMPTS} attempts: {}",
            last_error.unwrap_or_else(|| "unknown git worktree error".to_string())
        )
    })?;

    let worktree_root =
        fs::canonicalize(&target).context("failed to canonicalize subagent worktree")?;
    let child_workdir = worktree_root.join(relative_workdir);
    if !child_workdir.is_dir() {
        let _ = cleanup(&SubagentWorktree {
            repo_root: repo_root.clone(),
            parent_workdir: parent_workdir.clone(),
            worktree_root: worktree_root.clone(),
            workdir: child_workdir.clone(),
            branch_name: branch_name.clone(),
        });
        bail!(
            "worktree workdir does not exist: {}",
            display_path(&child_workdir)
        );
    }

    Ok(Some(SubagentWorktree {
        repo_root,
        parent_workdir,
        worktree_root,
        workdir: child_workdir,
        branch_name,
    }))
}

/// Apply isolated worktree changes into the parent repository checkout.
pub(crate) fn apply(worktree: &SubagentWorktree) -> Result<SubagentWorktreeApplyResult> {
    apply_worktree_changes(&worktree.parent_workdir, &worktree.worktree_root)
}

/// Remove the delegated worktree and its branch.
pub(crate) fn cleanup(worktree: &SubagentWorktree) -> SubagentWorktreeCleanupResult {
    cleanup_worktree_target(
        &worktree.worktree_root,
        Some(worktree.branch_name.as_str()),
        true,
        true,
    )
}

fn apply_worktree_changes(
    parent_workdir: &Path,
    worktree_root: &Path,
) -> Result<SubagentWorktreeApplyResult> {
    let parent_workdir = canonicalize_existing_dir(
        &display_path(parent_workdir),
        "parent workdir",
    )?;
    let worktree_root =
        canonicalize_existing_dir(&display_path(worktree_root), "worktree root")?;

    let parent_repo_root_raw = run_git(&parent_workdir, &["rev-parse", "--show-toplevel"]).map_err(|e| anyhow::anyhow!(e))?;
    let parent_repo_root =
        canonicalize_existing_dir(&parent_repo_root_raw, "parent git repo root")?;

    let parent_common_raw = run_git(&parent_workdir, &["rev-parse", "--git-common-dir"]).map_err(|e| anyhow::anyhow!(e))?;
    let worktree_common_raw = run_git(&worktree_root, &["rev-parse", "--git-common-dir"]).map_err(|e| anyhow::anyhow!(e))?;
    let parent_common =
        canonicalize_git_path(&parent_workdir, &parent_common_raw, "parent git common dir")?;
    let worktree_common =
        canonicalize_git_path(&worktree_root, &worktree_common_raw, "worktree git common dir")?;
    if parent_common != worktree_common {
        bail!("worktree does not belong to the same git repository as parent workdir");
    }

    let status = run_git(&worktree_root, &["status", "--short"]).map_err(|e| anyhow::anyhow!(e))?;
    if status.trim().is_empty() {
        return Ok(SubagentWorktreeApplyResult {
            applied: false,
            changed: false,
            status,
            patch_bytes: 0,
            skipped_reason: Some("no_changes".to_string()),
            apply_method: None,
            fallback_reason: None,
            copied_files: Vec::new(),
            deleted_files: Vec::new(),
            conflict_files: Vec::new(),
        });
    }

    let apply_paths = collect_apply_paths(&worktree_root).map_err(|e| anyhow::anyhow!(e))?;
    if apply_paths.is_empty() {
        return Ok(SubagentWorktreeApplyResult {
            applied: false,
            changed: true,
            status,
            patch_bytes: 0,
            skipped_reason: Some("no_applyable_changes".to_string()),
            apply_method: None,
            fallback_reason: None,
            copied_files: Vec::new(),
            deleted_files: Vec::new(),
            conflict_files: Vec::new(),
        });
    }

    stage_apply_paths(&worktree_root, &apply_paths).map_err(|e| anyhow::anyhow!(e))?;
    let patch = run_git_raw(
        &worktree_root,
        &["diff", "--cached", "--binary", "HEAD", "--"],
    )
    .map_err(|e| anyhow::anyhow!(e))?;
    let patch_bytes = patch.as_bytes().len();
    if patch.trim().is_empty() {
        return Ok(SubagentWorktreeApplyResult {
            applied: false,
            changed: true,
            status,
            patch_bytes,
            skipped_reason: Some("empty_patch".to_string()),
            apply_method: None,
            fallback_reason: None,
            copied_files: Vec::new(),
            deleted_files: Vec::new(),
            conflict_files: Vec::new(),
        });
    }

    match run_git_apply_with_options(&parent_repo_root, &patch, &[]) {
        Ok(()) => Ok(SubagentWorktreeApplyResult {
            applied: true,
            changed: true,
            status,
            patch_bytes,
            skipped_reason: None,
            apply_method: Some("git_apply".to_string()),
            fallback_reason: None,
            copied_files: Vec::new(),
            deleted_files: Vec::new(),
            conflict_files: Vec::new(),
        }),
        Err(apply_error) => {
            match run_git_apply_3way(&parent_repo_root, &patch) {
                Ok(()) => {
                    return Ok(SubagentWorktreeApplyResult {
                        applied: true,
                        changed: true,
                        status,
                        patch_bytes,
                        skipped_reason: None,
                        apply_method: Some("git_apply_3way".to_string()),
                        fallback_reason: Some(apply_error),
                        copied_files: Vec::new(),
                        deleted_files: Vec::new(),
                        conflict_files: Vec::new(),
                    });
                }
                Err(three_way_error) => {
                    let three_way_error = three_way_error;
            let fallback = apply_file_copy_fallback(&parent_repo_root, &worktree_root, &apply_paths)
                .map_err(|fallback_error| {
                    anyhow::anyhow!(
                        "git apply failed: {apply_error}; git apply --3way failed: {three_way_error}; file copy fallback failed:\n{fallback_error}"
                    )
                })?;
            let copied_or_deleted =
                !fallback.copied_files.is_empty() || !fallback.deleted_files.is_empty();
            Ok(SubagentWorktreeApplyResult {
                applied: copied_or_deleted,
                changed: true,
                status,
                patch_bytes,
                skipped_reason: if copied_or_deleted {
                    None
                } else if fallback.already_applied_count > 0 {
                    Some("already_applied".to_string())
                } else {
                    Some("fallback_noop".to_string())
                },
                apply_method: Some("file_copy_fallback".to_string()),
                fallback_reason: Some(format!(
                    "git apply failed: {apply_error}; git apply --3way failed: {three_way_error}"
                )),
                copied_files: fallback.copied_files,
                deleted_files: fallback.deleted_files,
                conflict_files: fallback.conflict_files,
            })
                }
            }
        }
    }
}

fn cleanup_worktree_target(
    worktree_root: &Path,
    branch_name: Option<&str>,
    force: bool,
    delete_branch: bool,
) -> SubagentWorktreeCleanupResult {
    let mut result = SubagentWorktreeCleanupResult {
        removed: false,
        branch_deleted: false,
        skipped_reason: None,
        error: None,
    };

    if !worktree_root.exists() {
        result.skipped_reason = Some("missing_worktree".to_string());
        return result;
    }

    let worktree_root = match fs::canonicalize(worktree_root) {
        Ok(path) => path,
        Err(err) => {
            result.error = Some(format!("failed to canonicalize worktree: {err}"));
            return result;
        }
    };
    if !is_sai_subagent_worktree(&worktree_root) {
        result.error = Some(format!(
            "refusing to cleanup non-Sai subagent worktree: {}",
            display_path(&worktree_root)
        ));
        return result;
    }

    let repo_cwd = collect_worktree_paths(&worktree_root)
        .ok()
        .and_then(|paths| {
            paths.into_iter().find(|candidate| {
                fs::canonicalize(candidate)
                    .map(|canonical| canonical != worktree_root)
                    .unwrap_or(false)
            })
        });

    let mut remove_args = vec!["worktree".to_string(), "remove".to_string()];
    if force {
        remove_args.push("--force".to_string());
    }
    remove_args.push(display_path(&worktree_root));

    match run_git_owned(&worktree_root, remove_args) {
        Ok(_) => result.removed = true,
        Err(git_error) => {
            if !force {
                result.error = Some(format!("git worktree remove failed: {git_error}"));
                return result;
            }
            if worktree_root.exists() {
                match fs::remove_dir_all(&worktree_root) {
                    Ok(_) => {
                        result.removed = true;
                        result.skipped_reason = Some("git_remove_failed_removed_dir".to_string());
                    }
                    Err(remove_err) => {
                        result.error = Some(format!(
                            "git worktree remove failed: {git_error}; remove_dir_all failed: {remove_err}"
                        ));
                        return result;
                    }
                }
            } else {
                result.removed = true;
            }
        }
    }

    if delete_branch {
        if let Some(branch) = normalize_sai_subagent_branch(branch_name) {
            if let Some(repo_cwd) = repo_cwd {
                match run_git_owned(
                    &repo_cwd,
                    vec!["branch".to_string(), "-D".to_string(), branch.clone()],
                ) {
                    Ok(_) => result.branch_deleted = true,
                    Err(err) => {
                        let lower = err.to_ascii_lowercase();
                        if lower.contains("not found") {
                            result
                                .skipped_reason
                                .get_or_insert_with(|| "branch_delete_skipped".to_string());
                        } else if lower.contains("checked out") {
                            result
                                .skipped_reason
                                .get_or_insert_with(|| "branch_delete_checked_out".to_string());
                        } else {
                            result.error = Some(format!(
                                "worktree removed, but branch delete failed for {branch}: {err}"
                            ));
                        }
                    }
                }
            } else {
                result
                    .skipped_reason
                    .get_or_insert_with(|| "branch_delete_no_repo_worktree".to_string());
            }
        } else if branch_name.is_some() {
            result
                .skipped_reason
                .get_or_insert_with(|| "branch_delete_not_sai_branch".to_string());
        }
    }

    result
}

#[derive(Debug)]
struct FileCopyFallbackResult {
    copied_files: Vec<String>,
    deleted_files: Vec<String>,
    conflict_files: Vec<String>,
    already_applied_count: usize,
}

enum FileCopyFallbackOp {
    Copy {
        rel_path: String,
        source: PathBuf,
        target: PathBuf,
    },
    Delete {
        rel_path: String,
        target: PathBuf,
    },
}

fn apply_file_copy_fallback(
    parent_repo_root: &Path,
    worktree_root: &Path,
    paths: &[String],
) -> Result<FileCopyFallbackResult, String> {
    let mut plan: Vec<FileCopyFallbackOp> = Vec::new();
    let mut conflicts = Vec::new();
    let mut already_applied_count = 0;

    for rel_path in paths {
        let rel_path = validate_git_relative_path(rel_path)?;
        let source = worktree_root.join(&rel_path);
        let target = parent_repo_root.join(&rel_path);

        if !source.exists() {
            let head_bytes = head_file_bytes(worktree_root, &rel_path)?;
            let Some(base_bytes) = head_bytes else {
                already_applied_count += 1;
                continue;
            };
            if !target.exists() {
                already_applied_count += 1;
                continue;
            }
            let target_meta = fs::symlink_metadata(&target)
                .map_err(|err| format!("failed to inspect fallback target {rel_path}: {err}"))?;
            if !target_meta.is_file() {
                conflicts.push(format!("{rel_path} (parent target is not a regular file)"));
                continue;
            }
            let target_bytes = fs::read(&target)
                .map_err(|err| format!("failed to read fallback target {rel_path}: {err}"))?;
            if target_bytes == base_bytes {
                plan.push(FileCopyFallbackOp::Delete { rel_path, target });
            } else {
                conflicts.push(format!("{rel_path} (parent file changed since HEAD)"));
            }
            continue;
        }

        let source_meta = fs::symlink_metadata(&source)
            .map_err(|err| format!("failed to inspect fallback source {rel_path}: {err}"))?;
        if !source_meta.is_file() {
            conflicts.push(format!("{rel_path} (non-file fallback is not supported)"));
            continue;
        }

        let source_bytes = fs::read(&source)
            .map_err(|err| format!("failed to read fallback source {rel_path}: {err}"))?;
        let head_bytes = head_file_bytes(worktree_root, &rel_path)?;

        if target.exists() {
            let target_meta = fs::symlink_metadata(&target)
                .map_err(|err| format!("failed to inspect fallback target {rel_path}: {err}"))?;
            if !target_meta.is_file() {
                conflicts.push(format!("{rel_path} (parent target is not a regular file)"));
                continue;
            }
            let target_bytes = fs::read(&target)
                .map_err(|err| format!("failed to read fallback target {rel_path}: {err}"))?;
            if target_bytes == source_bytes {
                already_applied_count += 1;
                continue;
            }
            match head_bytes {
                Some(base_bytes) if target_bytes == base_bytes => {
                    plan.push(FileCopyFallbackOp::Copy {
                        rel_path,
                        source,
                        target,
                    });
                }
                Some(_) => {
                    conflicts.push(format!("{rel_path} (parent file changed since HEAD)"));
                }
                None => {
                    conflicts.push(format!("{rel_path} (parent already has an untracked file)"));
                }
            }
        } else if head_bytes.is_some() {
            conflicts.push(format!(
                "{rel_path} (parent file is missing but exists in HEAD)"
            ));
        } else {
            plan.push(FileCopyFallbackOp::Copy {
                rel_path,
                source,
                target,
            });
        }
    }

    if !conflicts.is_empty() {
        return Err(conflicts.join("\n"));
    }

    let mut copied_files = Vec::new();
    let mut deleted_files = Vec::new();
    for operation in plan {
        match operation {
            FileCopyFallbackOp::Copy {
                rel_path,
                source,
                target,
            } => {
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent).map_err(|err| {
                        format!("failed to create fallback target directory: {err}")
                    })?;
                }
                fs::copy(&source, &target)
                    .map_err(|err| format!("failed to copy fallback file {rel_path}: {err}"))?;
                copied_files.push(rel_path);
            }
            FileCopyFallbackOp::Delete { rel_path, target } => {
                fs::remove_file(&target)
                    .map_err(|err| format!("failed to delete fallback file {rel_path}: {err}"))?;
                deleted_files.push(rel_path);
            }
        }
    }

    Ok(FileCopyFallbackResult {
        copied_files,
        deleted_files,
        conflict_files: Vec::new(),
        already_applied_count,
    })
}

fn stage_apply_paths(worktree_root: &Path, paths: &[String]) -> Result<(), String> {
    run_git(worktree_root, &["reset", "-q", "HEAD", "--"])?;
    if paths.is_empty() {
        return Ok(());
    }
    let mut args = vec!["add".to_string(), "-A".to_string(), "--".to_string()];
    args.extend(paths.iter().cloned());
    run_git_owned(worktree_root, args)?;
    Ok(())
}

fn head_file_bytes(repo_root: &Path, rel_path: &str) -> Result<Option<Vec<u8>>, String> {
    let head_spec = format!("HEAD:{rel_path}");
    if run_git_owned(
        repo_root,
        vec!["cat-file".to_string(), "-e".to_string(), head_spec.clone()],
    )
    .is_err()
    {
        return Ok(None);
    }
    run_git_owned_bytes(repo_root, vec!["show".to_string(), head_spec]).map(Some)
}

fn run_git_apply_with_options(cwd: &Path, patch: &str, options: &[&str]) -> Result<(), String> {
    let mut check_args = vec!["apply", "--check", "--whitespace=nowarn", "--binary"];
    check_args.extend(options.iter().copied());
    let mut apply_args = vec!["apply", "--whitespace=nowarn", "--binary"];
    apply_args.extend(options.iter().copied());
    run_git_with_input(cwd, &check_args, patch)
        .and_then(|_| run_git_with_input(cwd, &apply_args, patch))
        .map(|_| ())
}

fn run_git_apply_3way(cwd: &Path, patch: &str) -> Result<(), String> {
    let check_output = run_git_with_input_output(
        cwd,
        &[
            "apply",
            "--check",
            "--whitespace=nowarn",
            "--binary",
            "--3way",
        ],
        patch,
    )?;
    if check_output.to_ascii_lowercase().contains("with conflicts") {
        return Err(format!(
            "git apply --3way would leave conflicts:\n{check_output}"
        ));
    }
    run_git_with_input(
        cwd,
        &["apply", "--whitespace=nowarn", "--binary", "--3way"],
        patch,
    )
    .map(|_| ())
}

fn collect_apply_paths(worktree_root: &Path) -> Result<Vec<String>, String> {
    let mut paths = BTreeSet::new();
    let tracked_raw = run_git_raw(
        worktree_root,
        &["diff", "--no-renames", "--name-only", "-z", "HEAD", "--"],
    )?;
    let untracked_raw =
        run_git_raw(worktree_root, &["ls-files", "--others", "--exclude-standard", "-z"])?;
    for raw in split_nul_paths(&tracked_raw).chain(split_nul_paths(&untracked_raw)) {
        if should_ignore_apply_path(raw) {
            continue;
        }
        paths.insert(validate_git_relative_path(raw)?);
    }
    Ok(paths.into_iter().collect())
}

fn collect_worktree_paths(cwd: &Path) -> Result<Vec<PathBuf>, String> {
    let raw = run_git_raw(cwd, &["worktree", "list", "--porcelain"])?;
    let mut paths = Vec::new();
    for line in raw.lines() {
        let Some(path) = line.strip_prefix("worktree ") else {
            continue;
        };
        let path = PathBuf::from(path.trim());
        if path.is_absolute() {
            paths.push(path);
        }
    }
    Ok(paths)
}

fn is_sai_subagent_worktree(path: &Path) -> bool {
    path.components().any(|component| match component {
        Component::Normal(name) => name == WORKTREE_DIR_MARKER,
        _ => false,
    })
}

fn normalize_sai_subagent_branch(branch_name: Option<&str>) -> Option<String> {
    let branch = branch_name?.trim();
    if branch.starts_with(BRANCH_PREFIX) {
        Some(branch.to_string())
    } else {
        None
    }
}

fn should_ignore_apply_path(path: &str) -> bool {
    let file_name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    matches!(file_name, ".DS_Store" | "Thumbs.db" | "Desktop.ini")
}

fn validate_git_relative_path(raw: &str) -> Result<String, String> {
    if raw.is_empty() {
        return Err("empty git path".to_string());
    }
    let path = Path::new(raw);
    if path.is_absolute() {
        return Err(format!("git path must be relative: {raw}"));
    }
    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            _ => return Err(format!("git path contains unsafe component: {raw}")),
        }
    }
    Ok(raw.to_string())
}

fn split_nul_paths(raw: &str) -> impl Iterator<Item = &str> {
    raw.split('\0').filter(|path| !path.is_empty())
}

fn run_git(cwd: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .output()
        .map_err(|err| format!("failed to run git: {err}"))?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
    }
    Err(git_error_message(&output))
}

fn run_git_raw(cwd: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .output()
        .map_err(|err| format!("failed to run git: {err}"))?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).to_string());
    }
    Err(git_error_message(&output))
}

fn run_git_owned(cwd: &Path, args: Vec<String>) -> Result<String, String> {
    let output = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .output()
        .map_err(|err| format!("failed to run git: {err}"))?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
    }
    Err(git_error_message(&output))
}

fn run_git_owned_bytes(cwd: &Path, args: Vec<String>) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .output()
        .map_err(|err| format!("failed to run git: {err}"))?;
    if output.status.success() {
        return Ok(output.stdout);
    }
    Err(git_error_message(&output))
}

fn run_git_with_input(cwd: &Path, args: &[&str], input: &str) -> Result<String, String> {
    let mut child = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("failed to run git: {err}"))?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(input.as_bytes())
            .map_err(|err| format!("failed to write git stdin: {err}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|err| format!("failed to wait for git: {err}"))?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
    }
    Err(git_error_message(&output))
}

fn run_git_with_input_output(cwd: &Path, args: &[&str], input: &str) -> Result<String, String> {
    let mut child = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("failed to run git: {err}"))?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(input.as_bytes())
            .map_err(|err| format!("failed to write git stdin: {err}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|err| format!("failed to wait for git: {err}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let combined = match (stdout.is_empty(), stderr.is_empty()) {
        (true, true) => String::new(),
        (false, true) => stdout,
        (true, false) => stderr,
        (false, false) => format!("{stdout}\n{stderr}"),
    };
    if output.status.success() {
        Ok(combined)
    } else if combined.is_empty() {
        Err(format!("git exited with status {}", output.status))
    } else {
        Err(combined)
    }
}

fn git_error_message(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let message = if stderr.is_empty() { stdout } else { stderr };
    if message.is_empty() {
        format!("git exited with status {}", output.status)
    } else {
        message
    }
}

fn canonicalize_existing_dir(raw: &str, label: &str) -> Result<PathBuf> {
    let path = PathBuf::from(raw.trim());
    if raw.trim().is_empty() {
        bail!("{label} is required");
    }
    let canonical = fs::canonicalize(&path)
        .with_context(|| format!("{label} must be an existing directory: {raw}"))?;
    if !canonical.is_dir() {
        bail!("{label} must be a directory: {raw}");
    }
    Ok(canonical)
}

fn canonicalize_git_path(cwd: &Path, raw: &str, label: &str) -> Result<PathBuf> {
    let path = PathBuf::from(raw.trim());
    let absolute = if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    };
    fs::canonicalize(&absolute)
        .with_context(|| format!("{label} must resolve to an existing path: {}", display_path(&absolute)))
}

fn sanitize_path_component(input: &str, fallback: &str) -> String {
    let mut out = String::new();
    for ch in input.trim().chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
            out.push(ch);
        } else {
            out.push('-');
        }
    }
    let compact = out
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let trimmed = compact
        .trim_matches(|ch| ch == '-' || ch == '.')
        .to_string();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.chars().take(80).collect()
    }
}

fn unique_worktree_suffix() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("{millis}-{}", uuid::Uuid::new_v4().simple())
}

fn is_worktree_name_collision(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("reference already exists")
        || lower.contains("already exists")
        || lower.contains("already checked out")
        || lower.contains("is a missing but already registered worktree")
}

/// 将路径格式化为 Git 可接受的路径字符串。
///
/// Windows 上会去掉 `\\?\` 扩展前缀，避免 worktree 路径无效。
fn display_path(path: &Path) -> String {
    let raw = path.to_string_lossy();
    #[cfg(windows)]
    {
        let trimmed = raw
            .strip_prefix(r"\\?\UNC\")
            .map(|rest| format!(r"\\{rest}"))
            .or_else(|| raw.strip_prefix(r"\\?\").map(|rest| rest.to_string()))
            .unwrap_or_else(|| raw.into_owned());
        return trimmed;
    }
    #[cfg(not(windows))]
    {
        raw.into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "sai-subagent-worktree-{label}-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ))
    }

    fn git(cwd: &Path, args: &[&str]) -> Result<String, String> {
        run_git_owned(cwd, args.iter().map(|arg| (*arg).to_string()).collect())
    }

    fn init_repo(root: &Path) -> Result<(), String> {
        fs::create_dir_all(root).map_err(|err| format!("failed to create repo: {err}"))?;
        git(root, &["init"])?;
        git(root, &["config", "user.email", "sai-test@example.com"])?;
        git(root, &["config", "user.name", "Sai Test"])?;
        git(root, &["config", "core.autocrlf", "false"])?;
        fs::write(root.join("README.md"), "base\n")
            .map_err(|err| format!("failed to write README: {err}"))?;
        git(root, &["add", "README.md"])?;
        git(root, &["commit", "-m", "init"])?;
        Ok(())
    }

    #[test]
    fn creates_applies_and_cleans_worktree() -> Result<(), String> {
        let root = temp_root("create-apply");
        let repo = root.join("repo");
        init_repo(&repo)?;

        let worktree = try_create(&repo, "edit")
            .map_err(|err| err.to_string())?
            .ok_or_else(|| "expected worktree".to_string())?;
        assert!(worktree.worktree_root.exists());
        assert!(is_sai_subagent_worktree(&worktree.worktree_root));

        fs::create_dir_all(worktree.workdir.join("test"))
            .map_err(|err| format!("failed to create test dir: {err}"))?;
        fs::write(
            worktree.workdir.join("test/agent.md"),
            "# Agent CRUD Test\n\n- status: done\n",
        )
        .map_err(|err| format!("failed to write worktree file: {err}"))?;

        let apply_result = apply(&worktree).map_err(|err| err.to_string())?;
        assert!(apply_result.applied);
        assert_eq!(
            fs::read_to_string(repo.join("test/agent.md"))
                .map_err(|err| format!("failed to read parent file: {err}"))?,
            "# Agent CRUD Test\n\n- status: done\n"
        );

        let cleanup_result = cleanup(&worktree);
        assert!(cleanup_result.removed);
        assert!(!worktree.worktree_root.exists());

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn skips_non_git_directory() {
        let temp = tempfile::tempdir().unwrap();
        let result = try_create(temp.path(), "agent").unwrap();
        assert!(result.is_none());
    }
}
