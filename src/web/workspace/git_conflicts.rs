use super::*;
use anyhow::{bail, Result};
use std::path::{Path, PathBuf};

const MAX_CONFLICT_FILE_BYTES: usize = 2 * 1024 * 1024;

/// 读取冲突文件在 base、ours、theirs 和工作树中的文本内容。
///
/// 参数:
/// - `root`: 当前工作区目录
/// - `path`: 仓库相对路径
///
/// 返回:
/// - Merge Editor 所需的冲突内容
pub(crate) async fn git_conflict(root: &Path, path: &str) -> Result<GitConflictContent> {
    let state = ensure_ready(root).await?;
    let path = validate_repo_relative_path(path)?;
    ensure_conflicted(&state, &path)?;
    let repo = Path::new(&state.repo_root);
    let (base, ours, theirs, current) = tokio::join!(
        read_index_stage(repo, 1, &path),
        read_index_stage(repo, 2, &path),
        read_index_stage(repo, 3, &path),
        read_worktree_file(repo, &path),
    );
    Ok(GitConflictContent {
        state,
        path,
        base: base?,
        ours: ours?,
        theirs: theirs?,
        current: current?,
    })
}

/// 选择 ours、theirs 或自定义内容解决冲突，并将结果加入暂存区。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `state`: 当前仓库状态
/// - `path`: 冲突文件路径
/// - `resolution`: ours、theirs 或 content
/// - `content`: content 模式写回文本
///
/// 返回:
/// - Git 命令输出
pub(super) async fn resolve_conflict(
    repo: &Path,
    state: &GitRepositoryState,
    path: Option<&str>,
    resolution: Option<&str>,
    content: Option<&str>,
) -> Result<GitOutput> {
    let path = validate_repo_relative_path(path.unwrap_or_default())?;
    ensure_conflicted(state, &path)?;
    let selected = resolution
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("conflict resolution cannot be empty"))?;
    let (resolved, needs_stage) = match selected {
        "ours" => checkout_conflict_side(repo, &path, "--ours", 2).await?,
        "theirs" => checkout_conflict_side(repo, &path, "--theirs", 3).await?,
        "content" => {
            write_conflict_content(repo, &path, content.unwrap_or_default()).await?;
            (empty_output(), true)
        }
        value => bail!("unsupported conflict resolution: {value}"),
    };
    if !needs_stage {
        return Ok(resolved);
    }
    let staged = git_success(repo, &["add", "-A", "--", &path]).await?;
    Ok(merge_outputs([resolved, staged]))
}

/// 检出冲突一侧；该侧删除文件时同步删除工作树路径。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `path`: 冲突文件路径
/// - `checkout_flag`: --ours 或 --theirs
/// - `stage`: 对应 index stage
///
/// 返回:
/// - Git 命令输出，以及是否仍需执行 git add
async fn checkout_conflict_side(
    repo: &Path,
    path: &str,
    checkout_flag: &str,
    stage: u8,
) -> Result<(GitOutput, bool)> {
    if index_stage_exists(repo, stage, path).await? {
        Ok((
            git_success(repo, &["checkout", checkout_flag, "--", path]).await?,
            true,
        ))
    } else {
        Ok((
            git_success(repo, &["rm", "-f", "--ignore-unmatch", "--", path]).await?,
            false,
        ))
    }
}

/// 确认路径属于当前冲突集合。
///
/// 参数:
/// - `state`: 当前仓库状态
/// - `path`: 仓库相对路径
///
/// 返回:
/// - 路径存在冲突时返回空结果
fn ensure_conflicted(state: &GitRepositoryState, path: &str) -> Result<()> {
    if state
        .entries
        .iter()
        .any(|entry| entry.path == path && entry.conflicted)
    {
        return Ok(());
    }
    bail!("path is not an unresolved conflict: {path}")
}

/// 读取 index 指定 stage 的文本内容。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `stage`: 1 为 base、2 为 ours、3 为 theirs
/// - `path`: 仓库相对路径
///
/// 返回:
/// - stage 存在时返回文本，删除侧返回空
async fn read_index_stage(repo: &Path, stage: u8, path: &str) -> Result<Option<String>> {
    if !index_stage_exists(repo, stage, path).await? {
        return Ok(None);
    }
    let revision = format!(":{stage}:{path}");
    let output = git_raw(repo, &["show", &revision]).await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let message = if stderr.is_empty() { stdout } else { stderr };
        bail!(if message.is_empty() {
            "git show failed while reading conflict stage".to_string()
        } else {
            message
        });
    }
    decode_conflict_text(&output.stdout).map(Some)
}

/// 判断冲突文件是否包含指定 index stage，区分删除侧与 Git 读取失败。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `stage`: 1 为 base、2 为 ours、3 为 theirs
/// - `path`: 仓库相对路径
///
/// 返回:
/// - 指定 stage 存在时返回 true
async fn index_stage_exists(repo: &Path, stage: u8, path: &str) -> Result<bool> {
    let output = git_success(repo, &["ls-files", "--unmerged", "-z", "--", path]).await?;
    Ok(output.stdout.split('\0').any(|record| {
        record
            .split_once('\t')
            .and_then(|(metadata, _)| metadata.split_whitespace().nth(2))
            .and_then(|value| value.parse::<u8>().ok())
            == Some(stage)
    }))
}

/// 读取工作树中的当前冲突文本。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `path`: 仓库相对路径
///
/// 返回:
/// - 文件不存在时返回空文本
async fn read_worktree_file(repo: &Path, path: &str) -> Result<String> {
    let absolute = repo.join(path);
    let metadata = match tokio::fs::symlink_metadata(&absolute).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(error) => return Err(error.into()),
    };
    if metadata.file_type().is_symlink() {
        bail!("symbolic link conflicts are not supported by the merge editor");
    }
    if !metadata.is_file() {
        bail!("conflict path is not a regular file");
    }
    let bytes = tokio::fs::read(absolute).await?;
    decode_conflict_text(&bytes)
}

/// 校验并解码 Merge Editor 文本。
///
/// 参数:
/// - `bytes`: 文件字节
///
/// 返回:
/// - UTF-8 文本
fn decode_conflict_text(bytes: &[u8]) -> Result<String> {
    if bytes.len() > MAX_CONFLICT_FILE_BYTES {
        bail!("conflict file exceeds the 2 MiB merge editor limit");
    }
    if bytes.contains(&0) {
        bail!("binary conflicts are not supported by the merge editor");
    }
    String::from_utf8(bytes.to_vec())
        .map_err(|_| anyhow::anyhow!("conflict file is not UTF-8 text"))
}

/// 安全写回 Merge Editor 结果，拒绝仓库外路径和符号链接。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `path`: 仓库相对路径
/// - `content`: 解决后的文本
///
/// 返回:
/// - 写入成功时返回空结果
async fn write_conflict_content(repo: &Path, path: &str, content: &str) -> Result<()> {
    if content.len() > MAX_CONFLICT_FILE_BYTES {
        bail!("resolved content exceeds the 2 MiB merge editor limit");
    }
    let absolute = repo.join(path);
    let parent = absolute
        .parent()
        .ok_or_else(|| anyhow::anyhow!("conflict path has no parent"))?;
    let canonical_repo = canonical_path(repo).await?;
    let canonical_parent = canonical_path(parent).await?;
    if !canonical_parent.starts_with(&canonical_repo) {
        bail!("conflict path escapes repository");
    }
    if tokio::fs::symlink_metadata(&absolute)
        .await
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        bail!("cannot resolve a conflict through a symbolic link");
    }
    tokio::fs::write(absolute, content).await?;
    Ok(())
}

/// 规范化安全检查使用的目录路径。
///
/// 参数:
/// - `path`: 待规范化路径
///
/// 返回:
/// - 绝对规范路径
async fn canonical_path(path: &Path) -> Result<PathBuf> {
    tokio::fs::canonicalize(path)
        .await
        .map_err(anyhow::Error::from)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    /// 验证 Merge Editor 不会跟随工作树中的符号链接读取仓库外内容。
    #[tokio::test]
    async fn rejects_symbolic_link_worktree_content() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        let outside = temp.path().join("outside.txt");
        tokio::fs::create_dir(&repo).await.unwrap();
        tokio::fs::write(&outside, "secret\n").await.unwrap();
        symlink(&outside, repo.join("conflict.txt")).unwrap();

        let error = read_worktree_file(&repo, "conflict.txt").await.unwrap_err();

        assert!(error.to_string().contains("symbolic link"));
    }
}
