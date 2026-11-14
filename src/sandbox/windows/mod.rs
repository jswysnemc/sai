mod acl;
mod handles;
mod identity;
mod journal;
mod process;

use super::SandboxPolicy;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::process::Command;

pub(crate) const WORKER_ARG: &str = "--internal-windows-sandbox";

/// 【Windows沙箱】【启动协议】由可信父进程传入，策略与命令不通过模型提供的隐藏字段重建。
#[derive(Serialize, Deserialize)]
struct Launch {
    policy: Option<SandboxPolicy>,
    shell: String,
    command: String,
}

/// 【Windows沙箱】【平台入口】创建可信启动器，启动器负责 LPAC、路径授权和退出码。
/// 参数：解析策略、Shell 和脚本；返回沿用父执行器管道和取消机制的命令。
pub(super) fn command(policy: &SandboxPolicy, shell: &str, script: &str) -> Result<Command> {
    use base64::Engine;
    let payload = base64::engine::general_purpose::STANDARD.encode(serde_json::to_vec(&Launch {
        policy: Some(policy.clone()),
        shell: shell.to_string(),
        command: script.to_string(),
    })?);
    let mut command = Command::new(worker_executable()?);
    command.arg(WORKER_ARG).arg(payload);
    Ok(command)
}

/// 【Windows沙箱】【运行入口】解析内部协议，先完成权限准备再创建受限子进程。
/// 参数：命令行携带的协议文本；返回真实退出码。
pub(crate) fn serve(payload: &str) -> Result<u32> {
    use base64::Engine;
    let launch: Launch =
        serde_json::from_slice(&base64::engine::general_purpose::STANDARD.decode(payload)?)?;
    let shell = resolve_shell(&launch.shell)?;
    let flavor = crate::platform::shell_selection::shell_flavor(shell.as_os_str());
    let cwd = match &launch.policy {
        Some(policy) => policy.cwd.clone(),
        None => std::env::current_dir()?,
    };
    let script = if flavor.uses_powershell_syntax() {
        let path = cwd.to_string_lossy();
        let path = path
            .strip_prefix(r"\\?\")
            .unwrap_or(&path)
            .replace('\'', "''");
        format!(
            "try {{ Set-Location -LiteralPath '{path}' -ErrorAction Stop }} catch {{ [Console]::Error.WriteLine($_); exit 125 }}; {}",
            launch.command
        )
    } else {
        launch.command.clone()
    };
    let args = crate::platform::shell_selection::script_args(flavor, &script);
    let Some(policy) = launch.policy else {
        return process::run(
            None,
            &mut identity::Capabilities::new(false)?,
            &shell,
            &args,
            &std::env::current_dir()?,
        );
    };
    journal::recover()?;
    let name = format!("sai.sandbox.{}", uuid::Uuid::new_v4().simple());
    let identity = identity::Identity::create(&name)?;
    let journal = journal::Journal::new(identity.sid_string()?, name)?;
    let _grants = acl::Grants::apply(identity.sid, journal, &policy, &shell)?;
    let mut capabilities = identity::Capabilities::new(policy.network)?;
    process::run(
        Some(&identity),
        &mut capabilities,
        &shell,
        &args,
        &policy.cwd,
    )
}

/// 【Windows沙箱】【Shell 定位】按现有优先级解析可信可执行文件，禁止从工作目录隐式查找程序。
/// 参数：配置的 Shell；返回绝对路径。
pub(crate) fn resolve_shell(configured: &str) -> Result<PathBuf> {
    let candidates = if configured.trim().is_empty() {
        vec!["pwsh.exe", "powershell.exe", "cmd.exe"]
    } else {
        vec![configured.trim()]
    };
    let path = std::env::var_os("PATH").unwrap_or_default();
    for candidate in candidates {
        let direct = PathBuf::from(candidate);
        if direct.is_absolute() && direct.is_file() {
            return Ok(direct);
        }
        for root in std::env::split_paths(&path).filter(|path| path.is_absolute()) {
            for file in [root.join(candidate), root.join(format!("{candidate}.exe"))] {
                if file.is_file() {
                    return Ok(file);
                }
            }
        }
    }
    anyhow::bail!("no configured Windows shell was found in absolute PATH entries")
}

/// 【Windows沙箱】【后端探测】实际创建 LPAC Shell 并验证可读与可写能力；无参数，返回检查结果。
pub(super) fn probe() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let cwd = temp.path().canonicalize()?;
    std::fs::write(cwd.join("probe.txt"), "sai-probe")?;
    let policy = SandboxPolicy {
        access: super::FileAccess::ReadOnly,
        cwd: cwd.clone(),
        writable_roots: Vec::new(),
        write_protected: Vec::new(),
        deny_read: Vec::new(),
        network: false,
        allowed_domains: Vec::new(),
        temp_dir: None,
        env: Vec::new(),
        scrubbed_env: Vec::new(),
    };
    let shell = PathBuf::from(std::env::var_os("SystemRoot").context("SystemRoot is unavailable")?)
        .join("System32/cmd.exe");
    let output = command(&policy, &shell.to_string_lossy(), "type probe.txt")?
        .into_std()
        .output()?;
    anyhow::ensure!(
        output.status.success() && String::from_utf8_lossy(&output.stdout).contains("sai-probe"),
        "LPAC process probe failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = command(
        &policy,
        &shell.to_string_lossy(),
        "echo forbidden> blocked.txt",
    )?
    .into_std()
    .output()?;
    anyhow::ensure!(
        !output.status.success() && !cwd.join("blocked.txt").exists(),
        "LPAC readonly probe failed"
    );
    Ok(())
}

/// 【Windows命令】【进程树包装】不启用文件隔离时仍在执行脚本前创建 Job，覆盖前台与 PTY。
/// 参数：Shell 和脚本文本；返回可信内部启动器命令。
pub(crate) fn unrestricted_command(shell: &str, script: &str) -> Result<Command> {
    use base64::Engine;
    let shell = resolve_shell(shell)?;
    let payload = base64::engine::general_purpose::STANDARD.encode(serde_json::to_vec(&Launch {
        policy: None,
        shell: shell.to_string_lossy().into_owned(),
        command: script.into(),
    })?);
    let mut command = Command::new(worker_executable()?);
    command.arg(WORKER_ARG).arg(payload);
    Ok(command)
}

/// 【Windows命令】【启动器定位】测试进程使用同一构建配置目录中的正式程序，其余使用当前程序。
/// 参数：无；返回可信启动器路径。
fn worker_executable() -> Result<PathBuf> {
    let current = std::env::current_exe()?;
    #[cfg(test)]
    {
        return super::test_worker_executable::resolve(&current);
    }
    #[cfg(not(test))]
    Ok(current)
}
