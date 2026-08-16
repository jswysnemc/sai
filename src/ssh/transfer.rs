//! 通过 SSH exec 通道传输中小文件。
//!
//! 仓库当前未引入 SFTP 客户端依赖（`russh-sftp`），因此这里用远端普遍存在的
//! `base64` 命令搭配 exec 通道完成文件读写：下载走 `base64 <file>` 读回 stdout，
//! 上传走 `base64 -d > <file>` 从 stdin 写入。base64 保证二进制安全，避免 shell
//! 对原始字节的干扰。
//!
//! 局限（留待后续用真正的 SFTP 子系统替代）：
//! - 仅适合不超过 [`MAX_TRANSFER_BYTES`] 的单个文件；
//! - 依赖远端存在 `base64` 命令；
//! - 不保留文件权限、属主、时间戳等元数据，也不支持目录递归。

use super::session::{self};
use crate::web::ssh::SshClientHandler;
use anyhow::{bail, Context, Result};
use base64::Engine;
use russh::client;
use russh::ChannelMsg;
use std::time::Duration;

/// 单次传输的文件大小上限。
pub(crate) const MAX_TRANSFER_BYTES: usize = 5 * 1024 * 1024;

/// 用单引号包裹路径，避免远端 Shell 对空格与元字符二次解释。
///
/// 参数:
/// - `value`: 原始路径
///
/// 返回:
/// - 可安全嵌入 Shell 命令的字面量
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

/// 从远端下载文件内容。
///
/// 参数:
/// - `handle`: 已认证的连接句柄
/// - `remote_path`: 远端文件路径
/// - `timeout`: 执行超时
///
/// 返回:
/// - 文件原始字节；超限或远端命令失败时报错
pub(crate) async fn download_file(
    handle: &client::Handle<SshClientHandler>,
    remote_path: &str,
    timeout: Duration,
) -> Result<Vec<u8>> {
    // base64 文本比原始数据大约膨胀 4/3，输出上限相应放大
    let cap = MAX_TRANSFER_BYTES / 3 * 4 + 16;
    let command = format!("base64 {}", shell_quote(remote_path));
    let output = session::exec_command(handle, &command, timeout, cap).await?;
    if output.stdout_truncated {
        bail!("remote file exceeds the {MAX_TRANSFER_BYTES}-byte transfer limit");
    }
    if output.exit_status != Some(0) {
        bail!(
            "remote base64 failed (exit {:?}): {}",
            output.exit_status,
            output.stderr.trim()
        );
    }
    // base64 输出可能含换行，解码前去除全部空白
    let cleaned: String = output.stdout.split_whitespace().collect();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(cleaned.as_bytes())
        .context("failed to decode remote file content")?;
    if bytes.len() > MAX_TRANSFER_BYTES {
        bail!("remote file exceeds the {MAX_TRANSFER_BYTES}-byte transfer limit");
    }
    Ok(bytes)
}

/// 向远端上传文件内容。
///
/// 参数:
/// - `handle`: 已认证的连接句柄
/// - `remote_path`: 远端目标路径
/// - `data`: 待写入的原始字节
/// - `timeout`: 执行超时
///
/// 返回:
/// - 写入成功；超限或远端命令失败时报错
pub(crate) async fn upload_file(
    handle: &client::Handle<SshClientHandler>,
    remote_path: &str,
    data: &[u8],
    timeout: Duration,
) -> Result<()> {
    if data.len() > MAX_TRANSFER_BYTES {
        bail!("local file exceeds the {MAX_TRANSFER_BYTES}-byte transfer limit");
    }
    let encoded = base64::engine::general_purpose::STANDARD.encode(data);
    let remote_path = remote_path.to_string();
    let run = async move {
        let mut channel = handle
            .channel_open_session()
            .await
            .context("failed to open SSH session channel")?;
        channel
            .exec(false, format!("base64 -d > {}", shell_quote(&remote_path)))
            .await
            .context("failed to start remote write")?;
        // 把 base64 文本写入远端 stdin 后关闭写方向，触发 base64 -d 落盘
        channel
            .data_bytes(encoded)
            .await
            .context("failed to stream file content")?;
        channel
            .eof()
            .await
            .context("failed to finish remote write")?;

        let mut exit_status = None;
        let mut stderr: Vec<u8> = Vec::new();
        while let Some(message) = channel.wait().await {
            match message {
                ChannelMsg::ExtendedData { data, ext } if ext == 1 => {
                    stderr.extend_from_slice(&data);
                }
                ChannelMsg::ExitStatus { exit_status: code } => exit_status = Some(code),
                ChannelMsg::Eof | ChannelMsg::Close => break,
                _ => {}
            }
        }
        if exit_status != Some(0) {
            bail!(
                "remote base64 -d failed (exit {:?}): {}",
                exit_status,
                String::from_utf8_lossy(&stderr).trim()
            );
        }
        Ok::<(), anyhow::Error>(())
    };
    tokio::time::timeout(timeout, run)
        .await
        .context("file upload timed out")?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_paths_with_spaces_and_metacharacters() {
        assert_eq!(shell_quote("/srv/app"), "'/srv/app'");
        assert_eq!(shell_quote("/srv/a b;rm -rf /"), "'/srv/a b;rm -rf /'");
    }

    #[test]
    fn escapes_embedded_single_quotes() {
        assert_eq!(shell_quote("/srv/it's"), r"'/srv/it'\''s'");
    }
}
