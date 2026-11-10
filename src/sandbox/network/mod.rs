#[cfg(target_os = "linux")]
mod linux_socket_filter;
mod policy;
mod proxy;
#[cfg(unix)]
mod unix;

use super::SandboxPolicy;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

pub(crate) const WORKER_ARG: &str = "--internal-network-sandbox";
pub(crate) const BRIDGE_ARG: &str = "--internal-network-bridge";

#[derive(Serialize, Deserialize)]
pub(super) struct Launch {
    policy: SandboxPolicy,
    shell: String,
    script: String,
}

/// 【网络授权】【启动包装】校验规则后建立隔离代理启动器，平台不支持时拒绝执行。
/// 参数：完整策略、Shell 和脚本；返回受控命令。
pub(super) fn command(policy: &SandboxPolicy, shell: &str, script: &str) -> Result<Command> {
    policy::rules(&policy.allowed_domains)?;
    anyhow::ensure!(
        cfg!(any(target_os = "linux", target_os = "macos")),
        "domain-restricted command networking is not available on this platform"
    );
    use base64::Engine;
    let payload = base64::engine::general_purpose::STANDARD.encode(serde_json::to_vec(&Launch {
        policy: policy.clone(),
        shell: shell.into(),
        script: script.into(),
    })?);
    let mut command = Command::new(std::env::current_exe()?);
    command.arg(WORKER_ARG).arg(payload);
    Ok(command)
}

/// 【网络授权】【启动协议】处理内部网络入口；参数为入口和载荷，返回退出码。
pub(crate) async fn serve(kind: &str, payload: &str) -> Result<i32> {
    #[cfg(unix)]
    {
        if kind == BRIDGE_ARG {
            return unix::bridge(payload).await;
        }
        use base64::Engine;
        let launch: Launch =
            serde_json::from_slice(&base64::engine::general_purpose::STANDARD.decode(payload)?)?;
        unix::run(launch).await
    }
    #[cfg(not(unix))]
    {
        let _ = (kind, payload);
        anyhow::bail!("restricted network proxy is unavailable on this platform")
    }
}

/// 【网络授权】【代理环境】覆盖所有常见代理与绕过变量；参数为命令和本次端口，返回无。
fn proxy_env(command: &mut Command, port: u16) {
    let address = format!("http://127.0.0.1:{port}");
    for name in [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
    ] {
        command.env(name, &address);
    }
    command.env("NO_PROXY", "").env("no_proxy", "");
}

#[cfg(test)]
mod tests;
