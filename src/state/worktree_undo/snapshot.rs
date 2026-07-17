use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;

const SNAPSHOT_DIR: &str = "worktree-undo";

#[derive(Debug)]
pub(super) struct PendingSnapshot {
    pub(super) directory: PathBuf,
    pub(super) repository_root: PathBuf,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct SnapshotRecord {
    pub(super) repository_root: String,
    pub(super) head: String,
    pub(super) before_fingerprint: String,
    pub(super) after_fingerprint: String,
    pub(super) turn_id: String,
    pub(super) untracked: Vec<UntrackedEntry>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct UntrackedEntry {
    pub(super) path: String,
    pub(super) kind: UntrackedKind,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum UntrackedKind {
    File,
    Symlink,
}

/// 创建运行前工作树快照；非 Git 工作区返回空。
///
/// 参数:
/// - `state_dir`: 当前会话状态目录
/// - `workspace`: 当前工作目录
/// - `turn_id`: 当前轮次标识
///
/// 返回:
/// - 可选待完成快照
pub(super) fn start_snapshot(
    state_dir: &Path,
    workspace: &Path,
    turn_id: &str,
) -> Result<Option<PendingSnapshot>> {
    let Some(repository_root) = repository_root(workspace)? else {
        return Ok(None);
    };
    let root = state_dir.join(SNAPSHOT_DIR);
    std::fs::create_dir_all(&root)?;
    let directory = root.join(format!("pending_{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(directory.join("untracked"))?;
    let Some(head) = current_head(&repository_root)? else {
        return Ok(None);
    };
    let index_patch = git_bytes(
        &repository_root,
        &["diff", "--binary", "--cached", "HEAD", "--"],
    )?;
    let worktree_patch = git_bytes(&repository_root, &["diff", "--binary", "--"])?;
    std::fs::write(directory.join("before-index.patch"), index_patch)?;
    std::fs::write(directory.join("before-worktree.patch"), worktree_patch)?;
    let untracked = snapshot_untracked(&repository_root, &directory.join("untracked"))?;
    let record = SnapshotRecord {
        repository_root: repository_root.display().to_string(),
        head,
        before_fingerprint: worktree_fingerprint(&repository_root)?,
        after_fingerprint: String::new(),
        turn_id: turn_id.to_string(),
        untracked,
    };
    save_record(&directory, &record)?;
    Ok(Some(PendingSnapshot {
        directory,
        repository_root,
    }))
}

/// 删除属于指定轮次的最新或未完成快照。
///
/// 参数:
/// - `state_dir`: 当前会话状态目录
/// - `turn_id`: 当前轮次标识
///
/// 返回:
/// - 删除是否成功
pub(super) fn discard_snapshot(state_dir: &Path, turn_id: &str) -> Result<()> {
    let root = state_dir.join(SNAPSHOT_DIR);
    if !root.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(&root)? {
        let directory = entry?.path();
        if !directory.is_dir() {
            continue;
        }
        let Ok(record) = load_record(&directory) else {
            continue;
        };
        if record.turn_id == turn_id {
            std::fs::remove_dir_all(directory)?;
        }
    }
    Ok(())
}

/// 保存运行后指纹并将快照固化为轮次快照。
///
/// 参数:
/// - `pending`: 待完成快照
///
/// 返回:
/// - 固化是否成功
pub(super) fn finalize_snapshot(pending: PendingSnapshot) -> Result<()> {
    let mut record = load_record(&pending.directory)?;
    record.after_fingerprint = worktree_fingerprint(&pending.repository_root)?;
    save_record(&pending.directory, &record)?;
    let root = pending
        .directory
        .parent()
        .context("worktree snapshot has no parent")?;
    let finalized = snapshot_directory_from_root(root, &record.turn_id);
    if finalized.exists() {
        std::fs::remove_dir_all(&finalized)?;
    }
    std::fs::rename(&pending.directory, finalized)?;
    Ok(())
}

/// 返回指定轮次的固化快照目录。
pub(super) fn snapshot_directory(state_dir: &Path, turn_id: &str) -> PathBuf {
    snapshot_directory_from_root(&state_dir.join(SNAPSHOT_DIR), turn_id)
}

/// 读取快照记录。
pub(super) fn load_record(directory: &Path) -> Result<SnapshotRecord> {
    Ok(serde_json::from_slice(&std::fs::read(
        directory.join("record.json"),
    )?)?)
}

/// 计算索引、工作树和未跟踪文件的联合指纹。
pub(super) fn worktree_fingerprint(repository_root: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    hasher.update(git_bytes(
        repository_root,
        &["diff", "--binary", "--cached", "HEAD", "--"],
    )?);
    hasher.update(git_bytes(repository_root, &["diff", "--binary", "--"])?);
    for path in untracked_paths(repository_root)? {
        hasher.update(path.as_bytes());
        let absolute = repository_root.join(&path);
        let metadata = std::fs::symlink_metadata(&absolute)?;
        if metadata.file_type().is_symlink() {
            hasher.update(std::fs::read_link(absolute)?.to_string_lossy().as_bytes());
        } else {
            hasher.update(std::fs::read(absolute)?);
        }
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// 执行 Git 命令并返回文本输出。
pub(super) fn git_text(repository_root: &Path, args: &[&str]) -> Result<String> {
    let output = git_bytes(repository_root, args)?;
    Ok(String::from_utf8(output)?.trim().to_string())
}

/// 执行 Git 命令并返回原始标准输出。
pub(super) fn git_bytes(repository_root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repository_root)
        .output()?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(output.stdout)
}

/// 查找工作目录所属 Git 仓库根目录。
///
/// 参数:
/// - `workspace`: 当前工作目录
///
/// 返回:
/// - 仓库根目录；未安装 Git 或当前目录不属于仓库时返回空
fn repository_root(workspace: &Path) -> Result<Option<PathBuf>> {
    repository_root_with_program(workspace, "git")
}

/// 使用指定 Git 程序查找工作目录所属仓库根目录。
///
/// 参数:
/// - `workspace`: 当前工作目录
/// - `program`: Git 程序名或路径
///
/// 返回:
/// - 仓库根目录；程序不存在或当前目录不属于仓库时返回空
fn repository_root_with_program(workspace: &Path, program: &str) -> Result<Option<PathBuf>> {
    // 1. Git 未安装时禁用工作树快照，不阻断普通聊天
    let output = match Command::new(program)
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(workspace)
        .output()
    {
        Ok(output) => output,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err.into()),
    };
    // 2. 当前目录不属于 Git 仓库时同样跳过快照
    if !output.status.success() {
        return Ok(None);
    }
    Ok(Some(PathBuf::from(
        String::from_utf8(output.stdout)?.trim(),
    )))
}

/// 读取当前 HEAD；尚无提交时返回空。
fn current_head(repository_root: &Path) -> Result<Option<String>> {
    let output = Command::new("git")
        .args(["rev-parse", "--verify", "HEAD"])
        .current_dir(repository_root)
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }
    Ok(Some(String::from_utf8(output.stdout)?.trim().to_string()))
}

/// 复制运行前全部未跟踪文件并记录类型。
fn snapshot_untracked(repository_root: &Path, target: &Path) -> Result<Vec<UntrackedEntry>> {
    let mut entries = Vec::new();
    for path in untracked_paths(repository_root)? {
        let source = repository_root.join(&path);
        let destination = target.join(&path);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let metadata = std::fs::symlink_metadata(&source)?;
        let kind = if metadata.file_type().is_symlink() {
            std::fs::write(
                &destination,
                std::fs::read_link(&source)?.to_string_lossy().as_bytes(),
            )?;
            UntrackedKind::Symlink
        } else {
            std::fs::copy(&source, &destination)?;
            UntrackedKind::File
        };
        entries.push(UntrackedEntry { path, kind });
    }
    Ok(entries)
}

/// 读取全部未跟踪文件相对路径。
fn untracked_paths(repository_root: &Path) -> Result<Vec<String>> {
    let output = git_bytes(
        repository_root,
        &["ls-files", "--others", "--exclude-standard", "-z"],
    )?;
    Ok(output
        .split(|byte| *byte == 0)
        .filter(|value| !value.is_empty())
        .map(|value| String::from_utf8(value.to_vec()))
        .collect::<std::result::Result<Vec<_>, _>>()?)
}

/// 保存快照元数据记录。
fn save_record(directory: &Path, record: &SnapshotRecord) -> Result<()> {
    std::fs::write(
        directory.join("record.json"),
        serde_json::to_vec_pretty(record)?,
    )?;
    Ok(())
}

/// 根据轮次标识生成稳定且安全的快照目录名。
fn snapshot_directory_from_root(root: &Path, turn_id: &str) -> PathBuf {
    let mut hasher = Sha256::new();
    hasher.update(turn_id.as_bytes());
    root.join(format!("turn_{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::repository_root_with_program;

    /// 验证系统未安装 Git 时跳过工作树快照。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn missing_git_skips_repository_detection() {
        let temp = tempfile::tempdir().unwrap();

        let repository = repository_root_with_program(temp.path(), "sai-missing-git").unwrap();

        assert!(repository.is_none());
    }
}
