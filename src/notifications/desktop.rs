use super::delivery::DeliveryControl;
use anyhow::Result;

/// 【通知投递】【桌面接口】通过系统固定程序投递已校验的纯文本。
/// @param title 通知标题；body 为通知正文，均不拼接进命令或脚本源码
/// @returns 两秒内的投递结果；平台缺少桌面后端时返回错误
pub(super) fn send(title: &str, body: &str) -> Result<()> {
    send_controlled(
        title,
        body,
        &DeliveryControl::new(std::time::Duration::from_secs(2)),
    )
}

/// 【通知投递】【受控桌面接口】固定通知程序只接收数据参数，取消和超时会停止并回收子进程。
/// @param title 标题；body 为正文；control 为可信取消状态和时限
/// @returns 系统程序投递结果，缺少平台后端时明确报错
pub(super) fn send_controlled(title: &str, body: &str, control: &DeliveryControl) -> Result<()> {
    control.check()?;
    #[cfg(target_os = "linux")]
    {
        let mut command = std::process::Command::new("notify-send");
        command.args(["--app-name=Sai", "--urgency=normal", "--", title, body]);
        execute(command, control)?;
    }
    #[cfg(target_os = "macos")]
    {
        const SCRIPT: &str = "on run argv\ndisplay notification (item 2 of argv) with title (item 1 of argv)\nend run";
        let mut command = std::process::Command::new("osascript");
        command.args(["-e", SCRIPT, "--", title, body]);
        execute(command, control)?;
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (title, body);
        anyhow::bail!("desktop notification delivery is unavailable on this platform");
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    Ok(())
}

/// 【通知投递】【进程边界】限制平台通知程序的等待时间，关闭全部标准流。
/// @param command 已选定的固定系统通知程序；control 为可信取消状态和截止时间
/// @returns 执行成功、失败或超时；取消与超时后终止并回收子进程
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn execute(mut command: std::process::Command, control: &DeliveryControl) -> Result<()> {
    use std::process::Stdio;
    use std::time::Duration;

    // 1. 【通知投递】【进程启动】关闭标准流并在创建平台程序前检查调用状态
    control.check()?;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    // 2. 【通知投递】【进程等待】轮询退出与取消状态，不把平台失败报告为成功
    let result = (|| loop {
        control.check()?;
        match child.try_wait()? {
            Some(status) => {
                anyhow::ensure!(status.success(), "desktop notification program failed");
                return Ok(());
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    })();
    // 3. 【通知投递】【进程回收】错误、取消或超时后终止并等待直接子进程退出
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    result
}
