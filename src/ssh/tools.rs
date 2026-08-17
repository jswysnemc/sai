//! SSH 工具组处理函数。
//!
//! 面向模型暴露四个工具：列出主机、执行命令、上传文件、下载文件。所有涉及秘密的
//! 环节（私钥口令、主机指纹确认、高危命令确认）都经 [`super::secret`] 通道完成，
//! 秘密绝不出现在工具参数、结果或错误里；返回给模型的文本在离开前统一走
//! [`super::redact`] 脱敏。

use super::secret::{self, InteractiveKind, SecretResponse};
use super::{danger, redact, session, sudo, transfer};
use crate::config::{AppConfig, SshHostConfig};
use crate::paths::SaiPaths;
use crate::tools::{empty_parameters, ToolProgress, ToolRegistry, ToolSpec};
use crate::web::ssh::{trust_host_key, KnownHostStatus};
use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Duration;

/// 等待用户完成交互式征询的最长时间。
const INTERACTIVE_TIMEOUT: Duration = Duration::from_secs(180);
/// 建立连接时的最大重试轮数（指纹确认 + 口令输入各占一轮）。
const MAX_CONNECT_ATTEMPTS: usize = 6;

/// `ssh_run_command` 参数。
#[derive(Deserialize)]
struct RunCommandArgs {
    host_id: String,
    command: String,
    #[serde(default)]
    timeout_seconds: Option<u64>,
}

/// `ssh_upload_file` 参数。
#[derive(Deserialize)]
struct UploadArgs {
    host_id: String,
    local_path: String,
    remote_path: String,
    #[serde(default)]
    timeout_seconds: Option<u64>,
}

/// `ssh_download_file` 参数。
#[derive(Deserialize)]
struct DownloadArgs {
    host_id: String,
    remote_path: String,
    local_path: String,
    #[serde(default)]
    timeout_seconds: Option<u64>,
}

/// 注册 SSH 工具组。
///
/// 参数:
/// - `registry`: 目标工具注册表
/// - `paths`: Sai 路径集合，用于运行时加载最新主机配置
/// - `session_id`: 会话标识，交互征询按此路由到对应前端
///
/// 返回:
/// - 无
pub(crate) fn register(registry: &mut ToolRegistry, paths: &SaiPaths, session_id: &str) {
    register_list_hosts(registry, paths);
    register_run_command(registry, paths, session_id);
    register_upload_file(registry, paths, session_id);
    register_download_file(registry, paths, session_id);
}

/// 注册列出 SSH 主机工具（只读）。
fn register_list_hosts(registry: &mut ToolRegistry, paths: &SaiPaths) {
    let paths = paths.clone();
    registry.register(ToolSpec::new(
        "ssh_list_hosts",
        "List the SSH hosts configured for this workspace. Returns host aliases, connection targets (user@host), and whether a private key is configured. Credentials are never returned. Load this tool group before managing remote servers over SSH.",
        empty_parameters(),
        move |_args: Value| {
            let paths = paths.clone();
            async move { list_hosts(&paths) }
        },
    ));
}

/// 注册远程命令执行工具（写操作）。
fn register_run_command(registry: &mut ToolRegistry, paths: &SaiPaths, session_id: &str) {
    let paths = paths.clone();
    let session_id = session_id.to_string();
    registry.register(
        ToolSpec::new_with_progress(
            "ssh_run_command",
            "Run a shell command on a configured SSH host and return its exit status, stdout and stderr (each truncated to a byte cap). Reference the host only by its host_id from ssh_list_hosts. Passwords, key passphrases and host-key confirmations are collected through a secure UI and never pass through this tool. Dangerous commands (e.g. rm -rf, mkfs, systemctl stop) require an extra user confirmation.",
            json!({
                "type": "object",
                "properties": {
                    "host_id": {
                        "type": "string",
                        "description": "Host identifier returned by ssh_list_hosts."
                    },
                    "command": {
                        "type": "string",
                        "description": "Shell command to run on the remote host."
                    },
                    "timeout_seconds": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 600,
                        "description": "Optional command timeout in seconds (default 30, max 600)."
                    }
                },
                "required": ["host_id", "command"],
                "additionalProperties": false
            }),
            move |args, progress| {
                let paths = paths.clone();
                let session_id = session_id.clone();
                async move { run_command(args, progress, &paths, &session_id).await }
            },
        )
        .writes(),
    );
}

/// 注册文件上传工具（写操作）。
fn register_upload_file(registry: &mut ToolRegistry, paths: &SaiPaths, session_id: &str) {
    let paths = paths.clone();
    let session_id = session_id.to_string();
    registry.register(
        ToolSpec::new_with_progress(
            "ssh_upload_file",
            "Upload a local workspace file to a configured SSH host. The local path must stay inside the current workspace. Suitable for small to medium files only; metadata (permissions, owner, timestamps) is not preserved. Secrets are handled through a secure UI.",
            json!({
                "type": "object",
                "properties": {
                    "host_id": {"type": "string", "description": "Host identifier returned by ssh_list_hosts."},
                    "local_path": {"type": "string", "description": "Source file path relative to the workspace."},
                    "remote_path": {"type": "string", "description": "Absolute destination path on the remote host."},
                    "timeout_seconds": {"type": "integer", "minimum": 1, "maximum": 600, "description": "Optional timeout in seconds (default 30, max 600)."}
                },
                "required": ["host_id", "local_path", "remote_path"],
                "additionalProperties": false
            }),
            move |args, progress| {
                let paths = paths.clone();
                let session_id = session_id.clone();
                async move { upload_file(args, progress, &paths, &session_id).await }
            },
        )
        .writes(),
    );
}

/// 注册文件下载工具（写操作，落地到本地工作区）。
fn register_download_file(registry: &mut ToolRegistry, paths: &SaiPaths, session_id: &str) {
    let paths = paths.clone();
    let session_id = session_id.to_string();
    registry.register(
        ToolSpec::new_with_progress(
            "ssh_download_file",
            "Download a file from a configured SSH host into the current workspace. The destination path must stay inside the workspace. Suitable for small to medium files only. Secrets are handled through a secure UI.",
            json!({
                "type": "object",
                "properties": {
                    "host_id": {"type": "string", "description": "Host identifier returned by ssh_list_hosts."},
                    "remote_path": {"type": "string", "description": "Absolute source path on the remote host."},
                    "local_path": {"type": "string", "description": "Destination file path relative to the workspace."},
                    "timeout_seconds": {"type": "integer", "minimum": 1, "maximum": 600, "description": "Optional timeout in seconds (default 30, max 600)."}
                },
                "required": ["host_id", "remote_path", "local_path"],
                "additionalProperties": false
            }),
            move |args, progress| {
                let paths = paths.clone();
                let session_id = session_id.clone();
                async move { download_file(args, progress, &paths, &session_id).await }
            },
        )
        .writes(),
    );
}

/// 列出已配置的 SSH 主机（脱敏）。
///
/// 参数:
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 主机别名与连接目标的 JSON 文本，不含任何凭据
fn list_hosts(paths: &SaiPaths) -> Result<String> {
    let config = AppConfig::load_or_default(paths)?;
    let hosts = config
        .ssh
        .hosts
        .iter()
        .map(|host| {
            json!({
                "host_id": host.id,
                "label": host.label,
                "address": host.display_address(),
                "has_identity_file": !host.identity_file.trim().is_empty(),
                "remote_directory": host.remote_directory,
            })
        })
        .collect::<Vec<_>>();
    Ok(serde_json::to_string(&json!({ "hosts": hosts }))
        .unwrap_or_else(|_| "{\"hosts\":[]}".to_string()))
}

/// 按 host_id 加载主机配置。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `host_id`: 主机标识
///
/// 返回:
/// - 主机配置副本；未找到时报错
fn load_host(paths: &SaiPaths, host_id: &str) -> Result<SshHostConfig> {
    let config = AppConfig::load_or_default(paths)?;
    config
        .ssh
        .find(host_id)
        .cloned()
        .ok_or_else(|| anyhow!("unknown ssh host: {host_id}"))
}

/// 执行远程命令。
async fn run_command(
    args: Value,
    progress: ToolProgress,
    paths: &SaiPaths,
    session_id: &str,
) -> Result<String> {
    let args: RunCommandArgs =
        serde_json::from_value(args).context("invalid ssh_run_command arguments")?;
    let host = load_host(paths, &args.host_id)?;
    let timeout = session::clamp_timeout(args.timeout_seconds);

    // 1. 高危命令强制逐次确认，Yolo 模式同样不豁免
    if let Some(reason) = danger::dangerous_reason(&args.command) {
        let prompt = format!(
            "高危操作（{reason}）。确认在主机「{}」执行：{}",
            host.label, args.command
        );
        match request_interactive(
            session_id,
            &progress,
            InteractiveKind::DangerCommand,
            &host.label,
            &prompt,
            None,
            false,
        )
        .await?
        {
            SecretResponse::Confirmed(true) => {}
            _ => return Ok("用户拒绝执行该高危命令。".to_string()),
        }
    }

    // 2. sudo 密码由用户在安全输入里提供，模型看不到，也不进工具参数
    let mut stdin = None;
    if sudo::needs_sudo_password(&args.command) {
        match request_interactive(
            session_id,
            &progress,
            InteractiveKind::SudoPassword,
            &host.label,
            "远端命令需要 sudo 密码。密码只用于本次执行，不会交给模型。",
            None,
            false,
        )
        .await?
        {
            SecretResponse::Provided(secret) => {
                stdin = Some(format!("{secret}\n"));
            }
            _ => return Ok("用户取消了 sudo 密码输入。".to_string()),
        }
    }

    // 3. 建立连接（含指纹确认与口令输入），全程秘密不进模型
    let (handle, mut secrets) = establish_connection(session_id, &progress, &host).await?;
    if let Some(secret) = stdin.as_ref() {
        secrets.push(secret.trim_end_matches('\n').to_string());
    }

    // 4. 执行命令并对输出脱敏后返回
    let remote_command = if stdin.is_some() {
        sudo::with_sudo_stdin(&args.command)
    } else {
        args.command.clone()
    };
    let output = session::exec_command_with_stdin(
        &handle,
        &remote_command,
        timeout,
        session::MAX_OUTPUT_BYTES,
        stdin.as_deref().map(str::as_bytes),
    )
    .await
    .map_err(|error| anyhow!(redact::redact_error(&error, &secrets)))?;

    let payload = json!({
        "host_id": host.id,
        "exit_status": output.exit_status,
        "stdout": redact::redact(&output.stdout, &secrets),
        "stderr": redact::redact(&output.stderr, &secrets),
        "stdout_truncated": output.stdout_truncated,
        "stderr_truncated": output.stderr_truncated,
    });
    Ok(serde_json::to_string(&payload)?)
}

/// 上传本地工作区文件到远端。
async fn upload_file(
    args: Value,
    progress: ToolProgress,
    paths: &SaiPaths,
    session_id: &str,
) -> Result<String> {
    let args: UploadArgs =
        serde_json::from_value(args).context("invalid ssh_upload_file arguments")?;
    let host = load_host(paths, &args.host_id)?;
    let timeout = session::clamp_timeout(args.timeout_seconds);
    let local = resolve_workspace_path(&args.local_path)?;
    let data = std::fs::read(&local)
        .with_context(|| format!("failed to read local file {}", local.display()))?;

    let (handle, secrets) = establish_connection(session_id, &progress, &host).await?;
    transfer::upload_file(&handle, &args.remote_path, &data, timeout)
        .await
        .map_err(|error| anyhow!(redact::redact_error(&error, &secrets)))?;

    let payload = json!({
        "host_id": host.id,
        "uploaded": true,
        "bytes": data.len(),
        "remote_path": args.remote_path,
    });
    Ok(serde_json::to_string(&payload)?)
}

/// 从远端下载文件到本地工作区。
async fn download_file(
    args: Value,
    progress: ToolProgress,
    paths: &SaiPaths,
    session_id: &str,
) -> Result<String> {
    let args: DownloadArgs =
        serde_json::from_value(args).context("invalid ssh_download_file arguments")?;
    let host = load_host(paths, &args.host_id)?;
    let timeout = session::clamp_timeout(args.timeout_seconds);
    let local = resolve_workspace_path(&args.local_path)?;

    let (handle, secrets) = establish_connection(session_id, &progress, &host).await?;
    let data = transfer::download_file(&handle, &args.remote_path, timeout)
        .await
        .map_err(|error| anyhow!(redact::redact_error(&error, &secrets)))?;

    if let Some(parent) = local.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(&local, &data)
        .with_context(|| format!("failed to write local file {}", local.display()))?;

    let payload = json!({
        "host_id": host.id,
        "downloaded": true,
        "bytes": data.len(),
        "local_path": args.local_path,
    });
    Ok(serde_json::to_string(&payload)?)
}

/// 建立到远端主机的连接，按需完成指纹确认与口令输入。
///
/// 认证细节由后端处理：公钥无口令直连，私钥带口令时向前端安全征询口令，
/// 首次连接或指纹变更时请用户核对指纹。全过程秘密只在后端与前端之间流转。
///
/// 参数:
/// - `session_id`: 会话标识
/// - `progress`: 工具进度通道，用于发出交互带外标记
/// - `host`: 主机配置
///
/// 返回:
/// - 已认证的连接句柄，以及本次使用过的秘密集合（供输出脱敏）
async fn establish_connection(
    session_id: &str,
    progress: &ToolProgress,
    host: &SshHostConfig,
) -> Result<(
    russh::client::Handle<crate::web::ssh::SshClientHandler>,
    Vec<String>,
)> {
    let mut passphrase: Option<String> = None;
    let mut password: Option<String> = None;
    let mut secrets: Vec<String> = Vec::new();
    let mut passphrase_requested = false;
    let mut password_requested = false;

    for _ in 0..MAX_CONNECT_ATTEMPTS {
        match session::connect(host, passphrase.as_deref(), password.as_deref()).await {
            Ok(session::ConnectResult::Connected(handle)) => return Ok((handle, secrets)),
            Ok(session::ConnectResult::HostKeyPending { key, status }) => {
                // 指纹未登记或已变更：交给用户核对，绝不静默信任
                let changed = matches!(status, KnownHostStatus::Changed { .. });
                let prompt = if changed {
                    "远端主机指纹与既有记录不一致，存在中间人风险。确认后才会继续连接。".to_string()
                } else {
                    "首次连接该主机，请核对主机指纹后确认。".to_string()
                };
                match request_interactive(
                    session_id,
                    progress,
                    InteractiveKind::HostKey,
                    &host.label,
                    &prompt,
                    Some(key.fingerprint.clone()),
                    changed,
                )
                .await?
                {
                    SecretResponse::Confirmed(true) => {
                        trust_host_key(&key).context("failed to record the confirmed host key")?;
                    }
                    _ => bail!("用户未确认主机指纹，已取消连接。"),
                }
            }
            Err(error) => {
                // 公钥认证失败且疑似私钥带口令：向前端征询一次口令后重试
                if !passphrase_requested && needs_passphrase(&error) {
                    passphrase_requested = true;
                    match request_interactive(
                        session_id,
                        progress,
                        InteractiveKind::Passphrase,
                        &host.label,
                        "私钥已加密，请输入私钥口令。",
                        None,
                        false,
                    )
                    .await?
                    {
                        SecretResponse::Provided(secret) => {
                            secrets.push(secret.clone());
                            passphrase = Some(secret);
                        }
                        _ => bail!("用户取消了私钥口令输入。"),
                    }
                } else if !password_requested && needs_login_password(&error) {
                    password_requested = true;
                    match request_interactive(
                        session_id,
                        progress,
                        InteractiveKind::Password,
                        &host.label,
                        "公钥认证失败，请输入该主机的登录密码。",
                        None,
                        false,
                    )
                    .await?
                    {
                        SecretResponse::Provided(secret) => {
                            secrets.push(secret.clone());
                            password = Some(secret);
                        }
                        _ => bail!("用户取消了登录密码输入。"),
                    }
                } else {
                    return Err(anyhow!(redact::redact_error(&error, &secrets)));
                }
            }
        }
    }
    bail!("SSH 连接尝试次数过多，已放弃。")
}

/// 判断连接错误是否提示私钥需要口令。
///
/// 参数:
/// - `error`: 连接错误
///
/// 返回:
/// - 错误信息暗示私钥被加密时为 `true`
fn needs_passphrase(error: &anyhow::Error) -> bool {
    let message = format!("{error:#}").to_lowercase();
    message.contains("passphrase") || message.contains("encrypt") || message.contains("decrypt")
}

/// 判断连接错误是否适合改走登录密码。
fn needs_login_password(error: &anyhow::Error) -> bool {
    let message = format!("{error:#}").to_lowercase();
    message.contains("public key authentication failed")
        || message.contains("no ssh private key available")
        || message.contains("password authentication")
}

/// 发起一次交互式征询并等待应答。
///
/// 通过工具进度通道发出**不含秘密**的带外标记触发前端安全输入，秘密经独立通道
/// 直达后端。会话不支持交互或用户超时未响应时返回明确错误，避免工具永久阻塞。
///
/// 参数:
/// - `session_id`: 会话标识
/// - `progress`: 工具进度通道
/// - `kind`: 征询类型
/// - `host_label`: 主机别名
/// - `prompt`: 面向用户的中文提示
/// - `fingerprint`: 指纹（仅指纹确认时给出）
/// - `changed`: 指纹是否已变更
///
/// 返回:
/// - 用户应答
async fn request_interactive(
    session_id: &str,
    progress: &ToolProgress,
    kind: InteractiveKind,
    host_label: &str,
    prompt: &str,
    fingerprint: Option<String>,
    changed: bool,
) -> Result<SecretResponse> {
    let (request, receiver) =
        secret::request_secret(session_id, kind, host_label, prompt, fingerprint, changed);
    let id = request.id.clone();
    // 带外标记触发前端弹出安全输入；标记只含请求元信息
    progress.report(secret::encode_progress_marker(&request));
    let result = tokio::time::timeout(INTERACTIVE_TIMEOUT, receiver).await;
    // 结束标记让前端收起输入界面
    progress.report(secret::encode_resolved_marker(&id));
    match result {
        Ok(Ok(response)) => Ok(response),
        Ok(Err(_)) => Ok(SecretResponse::Cancelled),
        Err(_) => {
            // 超时后主动清理等待表，避免残留请求
            let _ = secret::submit_secret(&id, SecretResponse::Cancelled);
            bail!("未在限定时间内收到用户输入；当前会话可能不支持交互式秘密输入。")
        }
    }
}

/// 将相对路径解析到当前工作区内的绝对路径。
///
/// 上传的源文件与下载的落地文件都必须留在工作区内，防止读出工作区外的敏感文件
/// 或覆盖工作区外的系统文件。
///
/// 参数:
/// - `relative`: 相对工作区的路径
///
/// 返回:
/// - 工作区内的绝对路径；越界时报错
fn resolve_workspace_path(relative: &str) -> Result<PathBuf> {
    let workspace = crate::runtime_cwd::current_dir()?;
    let workspace = workspace.canonicalize().unwrap_or(workspace);
    let candidate = workspace.join(relative);

    // 逐段规整路径，剔除 `.` 与 `..`，避免借助 `..` 逃出工作区
    let mut normalized = PathBuf::new();
    for component in candidate.components() {
        use std::path::Component;
        match component {
            Component::ParentDir => {
                if !normalized.pop() {
                    bail!("path escapes the workspace: {relative}");
                }
            }
            Component::CurDir => {}
            other => normalized.push(other.as_os_str()),
        }
    }
    if !normalized.starts_with(&workspace) {
        bail!("path escapes the workspace: {relative}");
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_passphrase_detects_encrypted_key_errors() {
        assert!(needs_passphrase(&anyhow!(
            "SSH public key authentication failed - /home/u/.ssh/id_rsa: the key is encrypted"
        )));
        assert!(needs_passphrase(&anyhow!("wrong passphrase supplied")));
        assert!(!needs_passphrase(&anyhow!("connection refused")));
    }

    #[test]
    fn needs_login_password_detects_key_failure() {
        assert!(needs_login_password(&anyhow!(
            "SSH public key authentication failed - /home/u/.ssh/id_ed25519: rejected by the server"
        )));
        assert!(needs_login_password(&anyhow!(
            "no SSH private key available for deploy@box"
        )));
        assert!(!needs_login_password(&anyhow!("connection refused")));
    }

    #[test]
    fn resolve_workspace_path_rejects_parent_escape() {
        assert!(resolve_workspace_path("../etc/passwd").is_err());
        assert!(resolve_workspace_path("/etc/passwd").is_err());
    }

    #[test]
    fn resolve_workspace_path_accepts_inside_paths() {
        let resolved = resolve_workspace_path("sub/dir/file.txt").expect("应接受工作区内路径");
        assert!(resolved.ends_with("sub/dir/file.txt"));
    }
}
