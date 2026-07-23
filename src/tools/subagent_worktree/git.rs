use super::{BRANCH_PREFIX, WORKTREE_DIR_MARKER};
use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

/// 按应用路径重建 worktree 暂存区。
///
/// 参数:
/// - `worktree_root`: worktree 根目录
/// - `paths`: 需要暂存的相对路径
///
/// 返回:
/// - 成功时返回空值，失败时返回 Git 错误
pub(super) fn stage_apply_paths(worktree_root: &Path, paths: &[String]) -> Result<(), String> {
    run_git(worktree_root, &["reset", "-q", "HEAD", "--"])?;
    if paths.is_empty() {
        return Ok(());
    }
    let mut args = vec!["add".to_string(), "-A".to_string(), "--".to_string()];
    args.extend(paths.iter().cloned());
    run_git_owned(worktree_root, args)?;
    Ok(())
}

/// 读取 HEAD 中指定文件的原始字节。
///
/// 参数:
/// - `repo_root`: Git 仓库根目录
/// - `rel_path`: 仓库相对路径
///
/// 返回:
/// - 文件存在时返回字节，不存在时返回 `None`
pub(super) fn head_file_bytes(repo_root: &Path, rel_path: &str) -> Result<Option<Vec<u8>>, String> {
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

/// 检查并应用 Git 补丁。
///
/// 参数:
/// - `cwd`: Git 工作目录
/// - `patch`: 待应用补丁
/// - `options`: `git apply` 附加选项
///
/// 返回:
/// - 成功时返回空值，失败时返回 Git 错误
pub(super) fn run_git_apply_with_options(
    cwd: &Path,
    patch: &str,
    options: &[&str],
) -> Result<(), String> {
    let mut check_args = vec!["apply", "--check", "--whitespace=nowarn", "--binary"];
    check_args.extend(options.iter().copied());
    let mut apply_args = vec!["apply", "--whitespace=nowarn", "--binary"];
    apply_args.extend(options.iter().copied());
    run_git_with_input(cwd, &check_args, patch)
        .and_then(|_| run_git_with_input(cwd, &apply_args, patch))
        .map(|_| ())
}

/// 检查并以三方合并方式应用 Git 补丁。
///
/// 参数:
/// - `cwd`: Git 工作目录
/// - `patch`: 待应用补丁
///
/// 返回:
/// - 成功时返回空值，冲突或执行失败时返回错误
pub(super) fn run_git_apply_3way(cwd: &Path, patch: &str) -> Result<(), String> {
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

/// 收集 worktree 中需要回应到父工作区的路径。
///
/// 参数:
/// - `worktree_root`: worktree 根目录
///
/// 返回:
/// - 排序去重后的相对路径
pub(super) fn collect_apply_paths(worktree_root: &Path) -> Result<Vec<String>, String> {
    let mut paths = BTreeSet::new();
    let tracked_raw = run_git_raw(
        worktree_root,
        &["diff", "--no-renames", "--name-only", "-z", "HEAD", "--"],
    )?;
    let untracked_raw = run_git_raw(
        worktree_root,
        &["ls-files", "--others", "--exclude-standard", "-z"],
    )?;
    for raw in split_nul_paths(&tracked_raw).chain(split_nul_paths(&untracked_raw)) {
        if should_ignore_apply_path(raw) {
            continue;
        }
        paths.insert(validate_git_relative_path(raw)?);
    }
    Ok(paths.into_iter().collect())
}

/// 收集仓库已注册的 worktree 绝对路径。
///
/// 参数:
/// - `cwd`: Git 工作目录
///
/// 返回:
/// - worktree 绝对路径列表
pub(super) fn collect_worktree_paths(cwd: &Path) -> Result<Vec<PathBuf>, String> {
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

/// 判断路径是否位于 Sai 子智能体 worktree 目录中。
///
/// 参数:
/// - `path`: 待检查路径
///
/// 返回:
/// - 包含 worktree 目录标记时返回 `true`
pub(super) fn is_sai_subagent_worktree(path: &Path) -> bool {
    path.components().any(|component| match component {
        Component::Normal(name) => name == WORKTREE_DIR_MARKER,
        _ => false,
    })
}

/// 校验并规范化 Sai 子智能体分支名称。
///
/// 参数:
/// - `branch_name`: 可选分支名称
///
/// 返回:
/// - 合法分支名称，非 Sai 分支时返回 `None`
pub(super) fn normalize_sai_subagent_branch(branch_name: Option<&str>) -> Option<String> {
    let branch = branch_name?.trim();
    if branch.starts_with(BRANCH_PREFIX) {
        Some(branch.to_string())
    } else {
        None
    }
}

/// 判断应用路径是否属于平台元数据文件。
///
/// 参数:
/// - `path`: 仓库相对路径
///
/// 返回:
/// - 需要忽略时返回 `true`
fn should_ignore_apply_path(path: &str) -> bool {
    let file_name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    matches!(file_name, ".DS_Store" | "Thumbs.db" | "Desktop.ini")
}

/// 校验 Git 返回的仓库相对路径。
///
/// 参数:
/// - `raw`: 原始路径字符串
///
/// 返回:
/// - 安全的相对路径
pub(super) fn validate_git_relative_path(raw: &str) -> Result<String, String> {
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

/// 按 NUL 分隔符迭代 Git 路径输出。
///
/// 参数:
/// - `raw`: Git 原始输出
///
/// 返回:
/// - 非空路径迭代器
fn split_nul_paths(raw: &str) -> impl Iterator<Item = &str> {
    raw.split('\0').filter(|path| !path.is_empty())
}

/// 执行 Git 命令并返回去除边界空白的标准输出。
///
/// 参数:
/// - `cwd`: Git 工作目录
/// - `args`: Git 命令参数
///
/// 返回:
/// - 命令标准输出或错误文本
pub(super) fn run_git(cwd: &Path, args: &[&str]) -> Result<String, String> {
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

/// 执行 Git 命令并保留标准输出边界字符。
///
/// 参数:
/// - `cwd`: Git 工作目录
/// - `args`: Git 命令参数
///
/// 返回:
/// - 原始标准输出文本或错误文本
pub(super) fn run_git_raw(cwd: &Path, args: &[&str]) -> Result<String, String> {
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

/// 使用所有权参数执行 Git 命令。
///
/// 参数:
/// - `cwd`: Git 工作目录
/// - `args`: Git 命令参数
///
/// 返回:
/// - 去除边界空白的标准输出或错误文本
pub(super) fn run_git_owned(cwd: &Path, args: Vec<String>) -> Result<String, String> {
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

/// 使用所有权参数执行 Git 命令并返回原始字节。
///
/// 参数:
/// - `cwd`: Git 工作目录
/// - `args`: Git 命令参数
///
/// 返回:
/// - 原始标准输出字节或错误文本
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

/// 向 Git 子进程标准输入写入文本并执行命令。
///
/// 参数:
/// - `cwd`: Git 工作目录
/// - `args`: Git 命令参数
/// - `input`: 写入子进程的文本
///
/// 返回:
/// - 去除边界空白的标准输出或错误文本
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

/// 向 Git 子进程写入文本并合并标准输出与错误输出。
///
/// 参数:
/// - `cwd`: Git 工作目录
/// - `args`: Git 命令参数
/// - `input`: 写入子进程的文本
///
/// 返回:
/// - 合并后的命令输出或错误文本
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

/// 从 Git 子进程输出中提取错误文本。
///
/// 参数:
/// - `output`: 子进程输出
///
/// 返回:
/// - 错误输出、标准输出或退出状态文本
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
