use super::snapshot::{
    git_text, load_record, snapshot_directory, worktree_fingerprint, SnapshotRecord, UntrackedKind,
};
use super::WorktreeUndoOutcome;
use anyhow::{bail, Context, Result};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// 恢复当前会话最近一轮记录的 Git 工作树状态。
///
/// 参数:
/// - `state_dir`: 当前会话状态目录
/// - `expected_turn_id`: 预期撤销的最后轮次标识
///
/// 返回:
/// - 工作树撤销结果
pub(crate) fn restore_latest_snapshot(
    state_dir: &Path,
    expected_turn_id: &str,
) -> Result<WorktreeUndoOutcome> {
    let directory = snapshot_directory(state_dir, expected_turn_id);
    if !directory.exists() {
        return Ok(WorktreeUndoOutcome { restored: false });
    }
    let record = load_record(&directory)?;
    if record.turn_id != expected_turn_id {
        bail!("worktree snapshot does not belong to the last conversation turn");
    }
    let repository_root = PathBuf::from(&record.repository_root);
    validate_current_state(&repository_root, &record)?;
    restore_tracked_files(&repository_root)?;
    remove_current_untracked(&repository_root)?;
    apply_patch(
        &repository_root,
        &directory.join("before-index.patch"),
        true,
    )?;
    restore_worktree_from_index(&repository_root)?;
    apply_patch(
        &repository_root,
        &directory.join("before-worktree.patch"),
        false,
    )?;
    restore_untracked(&repository_root, &directory, &record)?;
    if worktree_fingerprint(&repository_root)? != record.before_fingerprint {
        bail!("worktree restore verification failed");
    }
    std::fs::remove_dir_all(directory)?;
    Ok(WorktreeUndoOutcome { restored: true })
}

/// 校验 HEAD 和运行后工作树指纹仍与快照一致。
///
/// 参数:
/// - `repository_root`: Git 仓库根目录
/// - `record`: 快照记录
///
/// 返回:
/// - 校验是否成功
fn validate_current_state(repository_root: &Path, record: &SnapshotRecord) -> Result<()> {
    if git_text(repository_root, &["rev-parse", "HEAD"])? != record.head {
        bail!("repository HEAD changed after the turn; undo was not applied");
    }
    if worktree_fingerprint(repository_root)? != record.after_fingerprint {
        bail!("worktree changed after the turn; undo was not applied");
    }
    Ok(())
}

/// 将索引和工作树中的跟踪文件恢复到当前 HEAD。
///
/// 参数:
/// - `repository_root`: Git 仓库根目录
///
/// 返回:
/// - 恢复是否成功
fn restore_tracked_files(repository_root: &Path) -> Result<()> {
    let status = Command::new("git")
        .args([
            "restore",
            "--source=HEAD",
            "--staged",
            "--worktree",
            "--",
            ".",
        ])
        .current_dir(repository_root)
        .status()?;
    if !status.success() {
        bail!("failed to restore tracked files to HEAD");
    }
    Ok(())
}

/// 删除当前全部未跟踪文件并清理空目录。
///
/// 参数:
/// - `repository_root`: Git 仓库根目录
///
/// 返回:
/// - 删除是否成功
fn remove_current_untracked(repository_root: &Path) -> Result<()> {
    let output = Command::new("git")
        .args(["ls-files", "--others", "--exclude-standard", "-z"])
        .current_dir(repository_root)
        .output()?;
    if !output.status.success() {
        bail!("failed to list untracked files before undo");
    }
    for path in output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|value| !value.is_empty())
    {
        let relative = String::from_utf8(path.to_vec())?;
        let absolute = repository_root.join(relative);
        if absolute.exists() || std::fs::symlink_metadata(&absolute).is_ok() {
            std::fs::remove_file(&absolute)?;
            prune_empty_parents(repository_root, absolute.parent())?;
        }
    }
    Ok(())
}

/// 将工作树内容恢复为当前索引内容。
///
/// 参数:
/// - `repository_root`: Git 仓库根目录
///
/// 返回:
/// - 恢复是否成功
fn restore_worktree_from_index(repository_root: &Path) -> Result<()> {
    let status = Command::new("git")
        .args(["restore", "--worktree", "--", "."])
        .current_dir(repository_root)
        .status()?;
    if !status.success() {
        bail!("failed to restore the worktree from the pre-turn index");
    }
    Ok(())
}

/// 应用快照中的索引补丁或工作树补丁。
///
/// 参数:
/// - `repository_root`: Git 仓库根目录
/// - `patch_file`: 补丁文件路径
/// - `cached`: 是否应用到索引
///
/// 返回:
/// - 应用是否成功
fn apply_patch(repository_root: &Path, patch_file: &Path, cached: bool) -> Result<()> {
    let patch = std::fs::read(patch_file)?;
    if patch.is_empty() {
        return Ok(());
    }
    let mut command = Command::new("git");
    command.args(["apply", "--binary", "--whitespace=nowarn"]);
    if cached {
        command.arg("--cached");
    }
    let mut child = command
        .arg("-")
        .current_dir(repository_root)
        .stdin(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .as_mut()
        .context("git apply stdin is unavailable")?
        .write_all(&patch)?;
    if !child.wait()?.success() {
        bail!("failed to apply the pre-turn worktree patch");
    }
    Ok(())
}

/// 恢复运行前存在的未跟踪文件与符号链接。
///
/// 参数:
/// - `repository_root`: Git 仓库根目录
/// - `directory`: 快照目录
/// - `record`: 快照记录
///
/// 返回:
/// - 恢复是否成功
fn restore_untracked(
    repository_root: &Path,
    directory: &Path,
    record: &SnapshotRecord,
) -> Result<()> {
    for entry in &record.untracked {
        let source = directory.join("untracked").join(&entry.path);
        let target = repository_root.join(&entry.path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        match entry.kind {
            UntrackedKind::File => {
                std::fs::copy(source, target)?;
            }
            UntrackedKind::Symlink => restore_symlink(&source, &target)?,
        }
    }
    Ok(())
}

#[cfg(unix)]
/// 在 Unix 平台恢复未跟踪符号链接。
fn restore_symlink(source: &Path, target: &Path) -> Result<()> {
    use std::os::unix::fs::symlink;
    symlink(std::fs::read_to_string(source)?, target)?;
    Ok(())
}

#[cfg(windows)]
/// 在 Windows 平台恢复未跟踪文件符号链接。
fn restore_symlink(source: &Path, target: &Path) -> Result<()> {
    use std::os::windows::fs::symlink_file;
    symlink_file(std::fs::read_to_string(source)?, target)?;
    Ok(())
}

/// 从指定目录向仓库根目录逐级清理空父目录。
///
/// 参数:
/// - `repository_root`: Git 仓库根目录
/// - `parent`: 起始父目录
///
/// 返回:
/// - 清理是否成功
fn prune_empty_parents(repository_root: &Path, mut parent: Option<&Path>) -> Result<()> {
    while let Some(directory) = parent {
        if directory == repository_root || std::fs::read_dir(directory)?.next().is_some() {
            break;
        }
        std::fs::remove_dir(directory)?;
        parent = directory.parent();
    }
    Ok(())
}
