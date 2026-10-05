use std::io::{self, BufRead, BufReader, IsTerminal, Write};
use std::process::{Command, Stdio};

use crate::cli::WebArgs;
use anyhow::{Context, Result};

/**
 * 【Web 服务】【后台启动】交互终端默认拆出子进程，只把监听地址打回当前终端。
 *
 * 参数:
 * - `args`: 已解析的 Web 参数
 *
 * 返回:
 * - 已拆出后台进程时为 true，调用方应立即退出
 */
pub(super) fn detach_if_interactive(args: &WebArgs) -> Result<bool> {
    if args.foreground || !io::stdout().is_terminal() {
        return Ok(false);
    }
    let mut command = Command::new(std::env::current_exe().context("locate sai executable")?);
    command
        .arg("web")
        .arg("--foreground")
        .arg("--host")
        .arg(&args.host)
        .arg("--port")
        .arg(args.port.to_string());
    if args.no_open {
        command.arg("--no-open");
    }
    if args.allow_anonymous {
        command.arg("--allow-anonymous");
    }
    if let Some(workspace) = &args.workspace {
        command.arg("--workspace").arg(workspace);
    }
    command
        .env("SAI_WEB_DETACHED", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    // 1. 【Web 服务】【进程组】新会话里继续运行，关闭终端后仍监听
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    let mut child = command.spawn().context("start Sai Web in the background")?;
    let stdout = child.stdout.take().context("read Sai Web address")?;
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    let mut address = String::new();
    while reader.read_line(&mut line).context("wait for Sai Web address")? > 0 {
        let trimmed = line.trim();
        if trimmed.contains("http://") || trimmed.contains("https://") {
            address = trimmed.to_string();
            break;
        }
        line.clear();
    }
    if address.is_empty() {
        anyhow::bail!("Sai Web started without printing a listen address");
    }
    writeln!(io::stdout(), "{address}")?;
    Ok(true)
}
