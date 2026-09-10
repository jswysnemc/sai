use anyhow::Result;

/// 【通知投递】【桌面接口】通过系统固定程序投递已校验的纯文本。
/// @param title 通知标题；body 为通知正文，均不拼接进命令或脚本源码
/// @returns 投递执行结果；Windows 沿用当前没有桌面通知后端的行为
pub(super) fn send(title: &str, body: &str) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        let mut command = std::process::Command::new("notify-send");
        command.args(["--app-name=Sai", "--urgency=normal", "--", title, body]);
        execute(command)?;
    }
    #[cfg(target_os = "macos")]
    {
        const SCRIPT: &str = "on run argv\ndisplay notification (item 2 of argv) with title (item 1 of argv)\nend run";
        let mut command = std::process::Command::new("osascript");
        command.args(["-e", SCRIPT, "--", title, body]);
        execute(command)?;
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let _ = (title, body);
    Ok(())
}

/// 【通知投递】【进程边界】限制平台通知程序的等待时间，关闭全部标准流。
/// @param command 已选定的固定系统通知程序
/// @returns 执行成功、失败或两秒超时；超时后终止并回收子进程
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn execute(mut command: std::process::Command) -> Result<()> {
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                anyhow::ensure!(status.success(), "desktop notification program failed");
                return Ok(());
            }
            Ok(None) => {}
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.into());
            }
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("desktop notification program timed out");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
