use super::host_store::{check_host_key, expand_home};
use super::known_hosts::{HostKey, KnownHostStatus};
use crate::config::SshHostConfig;
use anyhow::{Context, Result};
use base64::Engine;
use russh::client;
use russh::keys::ssh_key::HashAlg;
use russh::keys::{PrivateKeyWithHashAlg, PublicKeyBase64};
use std::sync::Arc;
use std::time::Duration;

/// 建立 SSH 连接的握手超时。
const SSH_CONNECT_TIMEOUT: Duration = Duration::from_secs(20);

/// 未显式指定私钥时依次尝试的默认私钥文件。
const DEFAULT_IDENTITY_FILES: [&str; 3] = ["id_ed25519", "id_ecdsa", "id_rsa"];

/// SSH 连接的结果。
pub(crate) enum SshConnectOutcome {
    /// 握手与认证均已完成
    Connected(client::Handle<SshClientHandler>),
    /// 主机密钥待用户确认，连接已中止
    HostKeyPending {
        key: Box<HostKey>,
        status: KnownHostStatus,
    },
}

/// russh 客户端回调，负责主机密钥校验。
pub(crate) struct SshClientHandler {
    hostname: String,
    port: u16,
    /// 校验未通过时在此留下密钥，供上层询问用户
    pending: Arc<std::sync::Mutex<Option<(HostKey, KnownHostStatus)>>>,
}

impl client::Handler for SshClientHandler {
    type Error = russh::Error;

    /// 校验远端主机密钥。
    ///
    /// 返回 false 会让 russh 中断握手。未知与变更两种情况都在此拦下，
    /// 由上层决定是提示用户确认指纹，还是按密钥变更告警。
    ///
    /// 参数:
    /// - `server_public_key`: 远端主机公钥
    ///
    /// 返回:
    /// - 是否信任该主机
    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        let key = HostKey {
            hostname: self.hostname.clone(),
            port: self.port,
            algorithm: server_public_key.algorithm().as_str().to_string(),
            key_base64: base64::engine::general_purpose::STANDARD
                .encode(server_public_key.public_key_bytes()),
            fingerprint: server_public_key.fingerprint(HashAlg::Sha256).to_string(),
        };
        let status = check_host_key(&key).unwrap_or(KnownHostStatus::Unknown);
        if status == KnownHostStatus::Known {
            return Ok(true);
        }
        if let Ok(mut pending) = self.pending.lock() {
            *pending = Some((key, status));
        }
        Ok(false)
    }
}

/// 连接远端主机并完成公钥认证。
///
/// 主机密钥未登记或已变更时不会继续认证，而是带回密钥信息由上层处理，
/// 避免把凭据发往未经确认的主机。
///
/// 参数:
/// - `host`: 主机配置
/// - `passphrase`: 私钥口令，无口令时传 None
///
/// 返回:
/// - 连接结果
pub(crate) async fn connect_ssh_session(
    host: &SshHostConfig,
    passphrase: Option<&str>,
) -> Result<SshConnectOutcome> {
    connect_ssh_session_auth(host, passphrase, None).await
}

/// 连接远端主机并完成认证，可附带登录密码。
///
/// 先尝试公钥；公钥全部失败且提供了登录密码时再试密码认证。
/// 密码只在本次调用中使用，不会写入配置或日志。
///
/// 参数:
/// - `host`: 主机配置
/// - `passphrase`: 私钥口令
/// - `password`: 登录密码，无则只走公钥
///
/// 返回:
/// - 连接结果
pub(crate) async fn connect_ssh_session_auth(
    host: &SshHostConfig,
    passphrase: Option<&str>,
    password: Option<&str>,
) -> Result<SshConnectOutcome> {
    let config = Arc::new(client::Config {
        inactivity_timeout: None,
        ..client::Config::default()
    });
    let pending = Arc::new(std::sync::Mutex::new(None));
    let handler = SshClientHandler {
        hostname: host.hostname.clone(),
        port: host.port,
        pending: pending.clone(),
    };

    // 1. 建立 TCP 连接并完成 SSH 握手，握手期间触发主机密钥校验
    let address = format!("{}:{}", host.hostname, host.port);
    let stream = tokio::time::timeout(
        SSH_CONNECT_TIMEOUT,
        tokio::net::TcpStream::connect(&address),
    )
    .await
    .with_context(|| format!("timed out connecting to {address}"))?
    .with_context(|| format!("failed to connect to {address}"))?;
    let handshake = tokio::time::timeout(
        SSH_CONNECT_TIMEOUT,
        client::connect_stream(config, stream, handler),
    )
    .await
    .with_context(|| format!("timed out during the SSH handshake with {address}"))?;

    let mut session = match handshake {
        Ok(session) => session,
        Err(error) => {
            // 2. 主机密钥未通过校验时握手必然失败，优先返回待确认的密钥
            let captured = pending.lock().ok().and_then(|mut slot| slot.take());
            if let Some((key, status)) = captured {
                return Ok(SshConnectOutcome::HostKeyPending {
                    key: Box::new(key),
                    status,
                });
            }
            return Err(error).with_context(|| format!("SSH handshake with {address} failed"));
        }
    };

    // 3. 依次尝试候选私钥，任一通过即完成认证
    let mut last_error = None;
    for path in identity_candidates(host) {
        let key = match russh::keys::load_secret_key(&path, passphrase) {
            Ok(key) => key,
            Err(error) => {
                last_error = Some(format!("{}: {error}", path.display()));
                continue;
            }
        };
        let key = PrivateKeyWithHashAlg::new(Arc::new(key), Some(HashAlg::Sha256));
        if session
            .authenticate_publickey(&host.username, key)
            .await
            .map_err(|error| anyhow::anyhow!("SSH authentication failed: {error}"))?
            .success()
        {
            return Ok(SshConnectOutcome::Connected(session));
        }
        last_error = Some(format!("{}: rejected by the server", path.display()));
    }

    // 4. 公钥都失败时，若用户已提供登录密码则再试密码认证
    if let Some(password) = password.filter(|value| !value.is_empty()) {
        if session
            .authenticate_password(&host.username, password)
            .await
            .map_err(|error| anyhow::anyhow!("SSH password authentication failed: {error}"))?
            .success()
        {
            return Ok(SshConnectOutcome::Connected(session));
        }
        anyhow::bail!("SSH password authentication was rejected");
    }

    match last_error {
        Some(reason) => anyhow::bail!("SSH public key authentication failed - {reason}"),
        None => anyhow::bail!(
            "no SSH private key available for {}",
            host.display_address()
        ),
    }
}

/// 列出该主机可用的私钥文件。
///
/// 参数:
/// - `host`: 主机配置
///
/// 返回:
/// - 按优先级排列且确实存在的私钥路径
fn identity_candidates(host: &SshHostConfig) -> Vec<std::path::PathBuf> {
    // 1. 显式配置的私钥优先，且不再回落到默认私钥
    if !host.identity_file.trim().is_empty() {
        return vec![expand_home(&host.identity_file)];
    }

    // 2. 未配置时按常见算法顺序尝试 ~/.ssh 下的默认私钥
    let Ok(base) = directories::BaseDirs::new()
        .map(|dirs| dirs.home_dir().join(".ssh"))
        .ok_or(())
    else {
        return Vec::new();
    };
    DEFAULT_IDENTITY_FILES
        .iter()
        .map(|name| base.join(name))
        .filter(|path| path.exists())
        .collect()
}
