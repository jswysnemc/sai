use super::known_hosts::{append_known_host, check_known_host, HostKey, KnownHostStatus};
use super::ssh_config_parser::parse_ssh_config;
use crate::config::SshHostConfig;
use anyhow::{Context, Result};
use std::path::PathBuf;

/// 可从 `~/.ssh/config` 导入的候选主机。
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct HostImportCandidate {
    pub label: String,
    pub hostname: String,
    pub port: u16,
    pub username: String,
    pub identity_file: String,
    /// 已存在同地址主机时为真，前端据此默认不勾选
    pub duplicate: bool,
}

/// 返回当前用户主目录。
fn home_dir() -> Result<PathBuf> {
    directories::BaseDirs::new()
        .map(|dirs| dirs.home_dir().to_path_buf())
        .context("cannot resolve the home directory")
}

/// 展开路径开头的 `~`。
///
/// `~/.ssh/config` 里的 IdentityFile 普遍写成波浪号形式，直接交给
/// 文件系统会被当作字面目录名。
///
/// 参数:
/// - `value`: 原始路径文本
///
/// 返回:
/// - 展开后的路径；无法解析主目录时按原样返回
pub(crate) fn expand_home(value: &str) -> PathBuf {
    let trimmed = value.trim();
    let rest = trimmed
        .strip_prefix("~/")
        .or_else(|| trimmed.strip_prefix("~\\"));
    match (rest, home_dir()) {
        (Some(rest), Ok(home)) => home.join(rest),
        _ => PathBuf::from(trimmed),
    }
}

/// 返回当前用户的 `.ssh` 目录。
fn ssh_dir() -> Result<PathBuf> {
    Ok(home_dir()?.join(".ssh"))
}

/// 读取 `~/.ssh/config` 文本。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 文件内容；文件不存在时返回空串
fn load_ssh_config_text() -> Result<String> {
    let path = ssh_dir()?.join("config");
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
    }
}

/// 解析 `~/.ssh/config` 并标注哪些主机已经存在。
///
/// 参数:
/// - `existing`: 已配置的主机列表
///
/// 返回:
/// - 候选主机列表
pub(crate) fn import_candidates(existing: &[SshHostConfig]) -> Result<Vec<HostImportCandidate>> {
    let text = load_ssh_config_text()?;
    Ok(parse_ssh_config(&text)
        .into_iter()
        .map(|candidate| {
            // 同用户、同主机、同端口视为重复，避免导入产生并列条目
            let duplicate = existing.iter().any(|host| {
                host.hostname == candidate.hostname
                    && host.port == candidate.port
                    && host.username == candidate.username
            });
            HostImportCandidate {
                label: candidate.alias,
                hostname: candidate.hostname,
                port: candidate.port,
                username: candidate.username,
                identity_file: candidate.identity_file,
                duplicate,
            }
        })
        .collect())
}

/// 读取 known_hosts 文本。
fn load_known_hosts() -> Result<String> {
    let path = ssh_dir()?.join("known_hosts");
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
    }
}

/// 校验远端主机密钥。
///
/// 参数:
/// - `key`: 远端返回的主机密钥
///
/// 返回:
/// - 校验结论
pub(crate) fn check_host_key(key: &HostKey) -> Result<KnownHostStatus> {
    Ok(check_known_host(&load_known_hosts()?, key))
}

/// 把用户确认过的主机密钥写入 known_hosts。
///
/// 指纹已变更的主机不允许在此覆盖：那种情况需要用户自行核实并手工处理，
/// 静默改写会让中间人攻击无声通过。
///
/// 参数:
/// - `key`: 待信任的主机密钥
///
/// 返回:
/// - 无
pub(crate) fn trust_host_key(key: &HostKey) -> Result<()> {
    let existing = load_known_hosts()?;
    match check_known_host(&existing, key) {
        KnownHostStatus::Known => return Ok(()),
        KnownHostStatus::Changed { .. } => {
            anyhow::bail!("host key has changed; resolve it in ~/.ssh/known_hosts first")
        }
        KnownHostStatus::Unknown => {}
    }

    // 1. 确保 .ssh 目录存在，并按 SSH 惯例收紧目录权限
    let dir = ssh_dir()?;
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create {}", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&dir)?.permissions();
        permissions.set_mode(0o700);
        let _ = std::fs::set_permissions(&dir, permissions);
    }

    // 2. 追加记录并把文件权限限制为仅本人可读写
    let path = dir.join("known_hosts");
    std::fs::write(&path, append_known_host(&existing, key))
        .with_context(|| format!("failed to write {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&path)?.permissions();
        permissions.set_mode(0o600);
        let _ = std::fs::set_permissions(&path, permissions);
    }
    Ok(())
}
