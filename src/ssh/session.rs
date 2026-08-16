//! SSH 连接与远程命令执行核心。
//!
//! 连接与认证完全复用 `crate::web::ssh` 的既有实现（公钥认证 + known_hosts 校验），
//! 本模块只补充"执行一次命令并带回受控输出"的能力：带超时、按流分别设输出上限。
//! 采用**按调用连接**策略——每次工具调用建立连接、执行、随句柄 drop 关闭连接。
//! 取舍：省去连接池的生命周期与并发管理，代价是每条命令一次握手开销；对 Agent 偶发的
//! 服务器管理动作足够，且不会残留长连接。后续如需高频批量执行可在此引入连接池。

use crate::config::SshHostConfig;
use crate::web::ssh::{
    connect_ssh_session, HostKey, KnownHostStatus, SshClientHandler, SshConnectOutcome,
};
use anyhow::{Context, Result};
use russh::client;
use russh::ChannelMsg;
use std::time::Duration;

/// 未显式指定时的默认命令超时。
pub(crate) const DEFAULT_COMMAND_TIMEOUT_SECS: u64 = 30;
/// 命令超时上限，防止模型给出过大的值把工具挂死。
pub(crate) const MAX_COMMAND_TIMEOUT_SECS: u64 = 600;
/// 单个输出流（stdout / stderr）的字节上限。
pub(crate) const MAX_OUTPUT_BYTES: usize = 64 * 1024;

/// 一次远程命令执行的受控结果。
pub(crate) struct RemoteCommandOutput {
    /// 远端返回的退出码，未收到时为 None
    pub(crate) exit_status: Option<u32>,
    /// 标准输出（可能被截断）
    pub(crate) stdout: String,
    /// 标准错误（可能被截断）
    pub(crate) stderr: String,
    /// 标准输出是否因超限被截断
    pub(crate) stdout_truncated: bool,
    /// 标准错误是否因超限被截断
    pub(crate) stderr_truncated: bool,
}

/// 连接尝试的结果。
pub(crate) enum ConnectResult {
    /// 握手与公钥认证均已完成
    Connected(client::Handle<SshClientHandler>),
    /// 主机密钥待用户确认，连接已中止
    HostKeyPending {
        key: Box<HostKey>,
        status: KnownHostStatus,
    },
}

/// 连接远端主机。
///
/// 参数:
/// - `host`: 主机配置
/// - `passphrase`: 私钥口令，无口令时传 None
///
/// 返回:
/// - 连接结果；主机密钥未确认时带回密钥供上层交互
pub(crate) async fn connect(
    host: &SshHostConfig,
    passphrase: Option<&str>,
) -> Result<ConnectResult> {
    match connect_ssh_session(host, passphrase).await? {
        SshConnectOutcome::Connected(handle) => Ok(ConnectResult::Connected(handle)),
        SshConnectOutcome::HostKeyPending { key, status } => {
            Ok(ConnectResult::HostKeyPending { key, status })
        }
    }
}

/// 在已建立的连接上执行一次命令并收集受控输出。
///
/// 参数:
/// - `handle`: 已认证的 SSH 连接句柄
/// - `command`: 远程命令原文
/// - `timeout`: 整体执行超时
/// - `max_bytes`: 每个输出流的字节上限
///
/// 返回:
/// - 命令的退出码与（可能截断的）标准输出、标准错误
pub(crate) async fn exec_command(
    handle: &client::Handle<SshClientHandler>,
    command: &str,
    timeout: Duration,
    max_bytes: usize,
) -> Result<RemoteCommandOutput> {
    let command = command.to_string();
    let run = async move {
        // 1. 打开会话通道并发起命令；exec 不请求回执，直接读取输出流
        let mut channel = handle
            .channel_open_session()
            .await
            .context("failed to open SSH session channel")?;
        channel
            .exec(false, command)
            .await
            .context("failed to start remote command")?;

        let mut stdout: Vec<u8> = Vec::new();
        let mut stderr: Vec<u8> = Vec::new();
        let mut stdout_truncated = false;
        let mut stderr_truncated = false;
        let mut exit_status = None;

        // 2. 读取通道消息，stdout 与 stderr 分别累积并各自设上限
        while let Some(message) = channel.wait().await {
            match message {
                ChannelMsg::Data { data } => {
                    append_capped(&mut stdout, &data, max_bytes, &mut stdout_truncated);
                }
                // ext == 1 为 SSH 约定的 stderr，其余扩展流忽略
                ChannelMsg::ExtendedData { data, ext } if ext == 1 => {
                    append_capped(&mut stderr, &data, max_bytes, &mut stderr_truncated);
                }
                ChannelMsg::ExitStatus { exit_status: code } => {
                    exit_status = Some(code);
                }
                ChannelMsg::Eof | ChannelMsg::Close => break,
                _ => {}
            }
            // 两个流都已截满时提前结束，避免为超大输出持续读取
            if stdout_truncated && stderr_truncated {
                break;
            }
        }

        Ok::<RemoteCommandOutput, anyhow::Error>(RemoteCommandOutput {
            exit_status,
            stdout: String::from_utf8_lossy(&stdout).into_owned(),
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
            stdout_truncated,
            stderr_truncated,
        })
    };

    tokio::time::timeout(timeout, run)
        .await
        .context("remote command timed out")?
}

/// 将数据追加到缓冲区，超过上限后停止追加并标记截断。
///
/// 参数:
/// - `buffer`: 目标缓冲区
/// - `data`: 新到达的数据块
/// - `max_bytes`: 缓冲区字节上限
/// - `truncated`: 截断标记，触发上限时置真
fn append_capped(buffer: &mut Vec<u8>, data: &[u8], max_bytes: usize, truncated: &mut bool) {
    if buffer.len() >= max_bytes {
        *truncated = true;
        return;
    }
    let remaining = max_bytes - buffer.len();
    if data.len() > remaining {
        buffer.extend_from_slice(&data[..remaining]);
        *truncated = true;
    } else {
        buffer.extend_from_slice(data);
    }
}

/// 将模型给出的超时秒数规整到安全范围。
///
/// 参数:
/// - `seconds`: 可选超时秒数
///
/// 返回:
/// - 位于 [1, 上限] 内的超时时长
pub(crate) fn clamp_timeout(seconds: Option<u64>) -> Duration {
    let seconds = seconds
        .unwrap_or(DEFAULT_COMMAND_TIMEOUT_SECS)
        .clamp(1, MAX_COMMAND_TIMEOUT_SECS);
    Duration::from_secs(seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_capped_stops_at_limit_and_flags_truncation() {
        let mut buffer = Vec::new();
        let mut truncated = false;
        append_capped(&mut buffer, b"hello", 3, &mut truncated);
        assert_eq!(buffer, b"hel");
        assert!(truncated);
    }

    #[test]
    fn append_capped_keeps_data_below_limit() {
        let mut buffer = Vec::new();
        let mut truncated = false;
        append_capped(&mut buffer, b"hi", 10, &mut truncated);
        assert_eq!(buffer, b"hi");
        assert!(!truncated);
    }

    #[test]
    fn append_capped_ignores_additional_data_once_full() {
        let mut buffer = b"abc".to_vec();
        let mut truncated = false;
        append_capped(&mut buffer, b"def", 3, &mut truncated);
        assert_eq!(buffer, b"abc");
        assert!(truncated);
    }

    #[test]
    fn clamp_timeout_applies_default_and_bounds() {
        assert_eq!(clamp_timeout(None).as_secs(), DEFAULT_COMMAND_TIMEOUT_SECS);
        assert_eq!(clamp_timeout(Some(0)).as_secs(), 1);
        assert_eq!(
            clamp_timeout(Some(100_000)).as_secs(),
            MAX_COMMAND_TIMEOUT_SECS
        );
        assert_eq!(clamp_timeout(Some(45)).as_secs(), 45);
    }

    /// 针对真实 SSH server 的端到端连接与执行验证。
    ///
    /// 默认忽略（需要外部 sshd）。通过环境变量启用：
    /// `SAI_SSH_IT_PORT`、`SAI_SSH_IT_USER`、`SAI_SSH_IT_KEY`。
    /// 建议配合隔离的临时 `HOME` 运行，避免污染用户的 known_hosts。
    #[tokio::test]
    #[ignore = "需要外部 SSH server，通过 SAI_SSH_IT_* 环境变量启用"]
    async fn real_exec_roundtrip() {
        let port: u16 = std::env::var("SAI_SSH_IT_PORT")
            .expect("SAI_SSH_IT_PORT")
            .parse()
            .expect("port");
        let username = std::env::var("SAI_SSH_IT_USER").expect("SAI_SSH_IT_USER");
        let identity_file = std::env::var("SAI_SSH_IT_KEY").expect("SAI_SSH_IT_KEY");
        let host = crate::config::SshHostConfig {
            id: "it".to_string(),
            label: "it".to_string(),
            hostname: "127.0.0.1".to_string(),
            port,
            username,
            identity_file,
            remote_directory: String::new(),
        };

        // 首次连接会带回待确认的主机指纹，信任后重连完成认证
        let handle = match connect(&host, None).await.expect("connect") {
            ConnectResult::Connected(handle) => handle,
            ConnectResult::HostKeyPending { key, .. } => {
                crate::web::ssh::trust_host_key(&key).expect("trust host key");
                match connect(&host, None).await.expect("reconnect") {
                    ConnectResult::Connected(handle) => handle,
                    ConnectResult::HostKeyPending { .. } => panic!("still pending after trust"),
                }
            }
        };

        let output = exec_command(
            &handle,
            "echo SAI_OK && echo SAI_ERR 1>&2 && exit 7",
            clamp_timeout(Some(10)),
            MAX_OUTPUT_BYTES,
        )
        .await
        .expect("exec");
        assert!(output.stdout.contains("SAI_OK"), "stdout={}", output.stdout);
        assert!(
            output.stderr.contains("SAI_ERR"),
            "stderr={}",
            output.stderr
        );
        assert_eq!(output.exit_status, Some(7));
    }
}
