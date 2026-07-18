use super::types::GitOutput;
use anyhow::{bail, Context, Result};
use std::path::Path;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::{sleep, timeout};

const GIT_COMMAND_TIMEOUT: Duration = Duration::from_secs(60);
const GIT_TRANSIENT_RETRY_ATTEMPTS: usize = 3;
const GIT_TRANSIENT_RETRY_DELAY: Duration = Duration::from_millis(160);

/// 执行 Git 命令并忽略标准输出。
///
/// 参数:
/// - `root`: Git 工作目录
/// - `args`: Git 参数
///
/// 返回:
/// - 命令成功时返回空结果
pub(super) async fn run_git(root: &Path, args: &[&str]) -> Result<()> {
    let _ = run_git_output(root, args).await?;
    Ok(())
}

/// 执行要求成功的 Git 命令。
///
/// 参数:
/// - `root`: Git 工作目录
/// - `args`: Git 参数
///
/// 返回:
/// - 清理后的标准输出和标准错误
pub(super) async fn run_git_output(root: &Path, args: &[&str]) -> Result<GitOutput> {
    let output = git_raw(root, args).await?;
    if output.status.success() {
        return Ok(GitOutput {
            stdout: trim_bytes(&output.stdout),
            stderr: trim_bytes(&output.stderr),
        });
    }
    let stderr = trim_bytes(&output.stderr);
    let stdout = trim_bytes(&output.stdout);
    let message = if stderr.is_empty() { stdout } else { stderr };
    if message.is_empty() {
        bail!("git command failed");
    }
    bail!("{message}")
}

/// 执行要求成功的 Git 命令。
///
/// 参数:
/// - `root`: Git 工作目录
/// - `args`: Git 参数
///
/// 返回:
/// - 清理后的命令输出
pub(super) async fn git_success(root: &Path, args: &[&str]) -> Result<GitOutput> {
    run_git_output(root, args).await
}

/// 执行 Git 子进程，并对临时锁错误进行有限重试。
///
/// 参数:
/// - `root`: Git 工作目录
/// - `args`: Git 参数
///
/// 返回:
/// - 原始进程输出
pub(super) async fn git_raw(root: &Path, args: &[&str]) -> Result<std::process::Output> {
    for attempt in 0..GIT_TRANSIENT_RETRY_ATTEMPTS {
        let output = run_git_once(root, args).await?;
        let message = format!(
            "{}\n{}",
            trim_bytes(&output.stderr),
            trim_bytes(&output.stdout)
        );
        let should_retry = !output.status.success()
            && attempt + 1 < GIT_TRANSIENT_RETRY_ATTEMPTS
            && is_transient_git_lock_error(&message);
        if !should_retry {
            return Ok(output);
        }
        sleep(GIT_TRANSIENT_RETRY_DELAY).await;
    }
    unreachable!("Git retry loop always returns on its final attempt")
}

/// 执行单次带超时的 Git 子进程。
///
/// 参数:
/// - `root`: Git 工作目录
/// - `args`: Git 参数
///
/// 返回:
/// - 原始进程输出
async fn run_git_once(root: &Path, args: &[&str]) -> Result<std::process::Output> {
    let mut command = Command::new("git");
    command
        .args(args)
        .current_dir(root)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("LC_ALL", "C")
        .kill_on_drop(true);
    timeout(GIT_COMMAND_TIMEOUT, command.output())
        .await
        .with_context(|| {
            format!(
                "git command timed out after 60 seconds: git {}",
                args.join(" ")
            )
        })?
        .context("failed to execute git command")
}

/// 判断错误是否来自可重试的 Git 临时锁。
///
/// 参数:
/// - `message`: Git 错误文本
///
/// 返回:
/// - 是否适合短暂等待后重试
fn is_transient_git_lock_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("another git process")
        || lower.contains("index.lock")
        || lower.contains("cannot lock ref")
        || lower.contains("could not lock")
        || (lower.contains("unable to create") && lower.contains(".lock"))
        || lower.contains("failed to lock")
}

/// 清理 Git 输出首尾空白。
///
/// 参数:
/// - `bytes`: Git 输出字节
///
/// 返回:
/// - 有损 UTF-8 文本
pub(super) fn trim_bytes(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::is_transient_git_lock_error;

    #[test]
    fn recognizes_transient_lock_errors() {
        assert!(is_transient_git_lock_error(
            "fatal: Unable to create '.git/index.lock': File exists"
        ));
        assert!(is_transient_git_lock_error(
            "cannot lock ref 'refs/heads/main'"
        ));
        assert!(!is_transient_git_lock_error(
            "fatal: bad revision 'missing'"
        ));
    }
}
