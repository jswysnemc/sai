use super::{policy, proxy, Launch};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{os::unix::fs::PermissionsExt, path::PathBuf, sync::Arc};
use tokio::{
    net::{TcpListener, UnixListener, UnixStream},
    process::Command,
    task::JoinSet,
};

#[derive(Serialize, Deserialize)]
struct Bridge {
    socket: PathBuf,
    shell: String,
    script: String,
}

/// 【网络授权】【主机代理】代理处于主机网络，实际命令只能访问本次代理出口。
/// 参数：可信策略和脚本；返回命令退出码，退出时取消全部转发。
pub(super) async fn run(mut launch: Launch) -> Result<i32> {
    let rules = Arc::new(policy::rules(&launch.policy.allowed_domains)?);
    launch.policy.network = false;
    let mut connections = JoinSet::new();
    #[cfg(target_os = "linux")]
    {
        let root = tempfile::tempdir()?;
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700))?;
        let socket = root.path().join("proxy.sock");
        let listener = UnixListener::bind(&socket)?;
        use base64::Engine;
        let bridge =
            base64::engine::general_purpose::STANDARD.encode(serde_json::to_vec(&Bridge {
                socket,
                shell: launch.shell,
                script: launch.script,
            })?);
        let mut args = super::super::linux_bwrap::bwrap_args(&launch.policy, "sh", "true");
        args.truncate(args.len() - 3);
        args.extend([
            std::env::current_exe()?.into_os_string(),
            super::BRIDGE_ARG.into(),
            bridge.into(),
        ]);
        let mut child = Command::new("bwrap")
            .args(args)
            .kill_on_drop(true)
            .spawn()?;
        loop {
            tokio::select! {
                status = child.wait() => return Ok(status?.code().unwrap_or(1)),
                incoming = listener.accept(), if connections.len() < 64 => {
                    let (stream, _) = incoming?;
                    let rules = rules.clone();
                    connections.spawn(async move { let _ = proxy::serve(stream, &rules).await; });
                }
                Some(_) = connections.join_next(), if !connections.is_empty() => {},
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        let mut profile = super::super::macos_seatbelt::seatbelt_profile(&launch.policy);
        profile.push_str(&format!(
            "(allow network-outbound (remote ip \"localhost:{port}\"))\n"
        ));
        let mut command = Command::new(super::super::macos_seatbelt::SANDBOX_EXEC);
        command
            .args(["-p", &profile, &launch.shell, "-lc", &launch.script])
            .current_dir(&launch.policy.cwd)
            .kill_on_drop(true);
        super::proxy_env(&mut command, port);
        let mut child = command.spawn()?;
        loop {
            tokio::select! {
                status = child.wait() => return Ok(status?.code().unwrap_or(1)),
                incoming = listener.accept(), if connections.len() < 64 => {
                    let (stream, _) = incoming?;
                    let rules = rules.clone();
                    connections.spawn(async move { let _ = proxy::serve(stream, &rules).await; });
                }
                Some(_) = connections.join_next(), if !connections.is_empty() => {},
            }
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    anyhow::bail!("restricted network isolation is not supported")
}

/// 【网络授权】【命名空间桥接】在断网命名空间监听回环，全部连接经 Unix 通道交给主机策略代理。
/// 参数：可信桥接载荷；返回脚本退出码，转发任务随脚本结束销毁。
pub(super) async fn bridge(payload: &str) -> Result<i32> {
    use base64::Engine;
    let request: Bridge =
        serde_json::from_slice(&base64::engine::general_purpose::STANDARD.decode(payload)?)?;
    anyhow::ensure!(
        request.socket.is_absolute(),
        "network broker socket must be absolute"
    );
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let mut command = Command::new(&request.shell);
    command.args(["-lc", &request.script]).kill_on_drop(true);
    super::proxy_env(&mut command, port);
    #[cfg(target_os = "linux")]
    super::linux_socket_filter::install(&mut command);
    let mut child = command
        .spawn()
        .context("start restricted network command")?;
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            status = child.wait() => return Ok(status?.code().unwrap_or(1)),
            incoming = listener.accept(), if connections.len() < 64 => {
                let (mut stream, _) = incoming?;
                let socket = request.socket.clone();
                connections.spawn(async move {
                    if let Ok(mut broker) = UnixStream::connect(socket).await { let _ = tokio::io::copy_bidirectional(&mut stream, &mut broker).await; }
                });
            }
            Some(_) = connections.join_next(), if !connections.is_empty() => {},
        }
    }
}
