//! 带超时的 cua-driver 版本探测。

use anyhow::{Context, Result};
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// 【桌面操作】【版本探测】执行 `cua-driver --version`，超时后结束子进程。
///
/// 参数:
/// - `path`: 驱动可执行文件
///
/// 返回:
/// - 解析出的主版本号
pub(crate) async fn probe_version(path: &Path) -> Result<String> {
    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    // 1. 启动 --version，工作目录设为二进制所在目录以便解析辅助文件
    let child = Command::new(path)
        .arg("--version")
        .current_dir(directory)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("failed to start {}", path.display()))?;
    // 2. 超时后由 kill_on_drop 结束子进程
    let output = tokio::time::timeout(PROBE_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| anyhow::anyhow!("timed out after 5s reading {}", path.display()))?
        .with_context(|| format!("failed to wait for {}", path.display()))?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    parse_semver(&text)
        .ok_or_else(|| anyhow::anyhow!("no version in driver output: {}", text.trim()))
}

/// 【桌面操作】【版本解析】提取文本中第一个 x.y.z 版本号。
///
/// 参数:
/// - `text`: 命令输出
///
/// 返回:
/// - 版本号
pub(crate) fn parse_semver(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index].is_ascii_digit() {
            let start = index;
            let mut dots = 0;
            while index < bytes.len() && (bytes[index].is_ascii_digit() || bytes[index] == b'.') {
                if bytes[index] == b'.' {
                    dots += 1;
                }
                index += 1;
            }
            let candidate = &text[start..index];
            if dots == 2 {
                let mut parts = candidate.split('.');
                if parts
                    .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
                {
                    return Some(candidate.to_string());
                }
            }
            continue;
        }
        index += 1;
    }
    None
}

/// 【桌面操作】【版本比较】判断版本是否达到最低要求。
///
/// 参数:
/// - `version`: 实际版本
/// - `minimum`: 最低版本
///
/// 返回:
/// - 达到或超过最低版本时为真
pub(crate) fn version_at_least(version: &str, minimum: &str) -> bool {
    match (semver_parts(version), semver_parts(minimum)) {
        (Some(left), Some(right)) => left >= right,
        _ => false,
    }
}

/// 【桌面操作】【版本拆分】把 x.y.z 拆成可比较的三段整数。
///
/// 参数:
/// - `version`: 版本号
///
/// 返回:
/// - 三段整数
fn semver_parts(version: &str) -> Option<(u64, u64, u64)> {
    let mut parts = version.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}
