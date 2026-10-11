//! Unix 作业控制：TUI 忽略停止信号并保持前台，子进程脱离控制终端。
//!
//! 子进程打开 `/dev/tty` 并调用 `tcsetpgrp` 后，控制终端的前台进程组可能停在
//! 已退出的子进程上。随后 TUI 读终端会收到 SIGTTIN，shell 显示
//! `zsh: suspended (tty input)`。进入 TUI 时忽略这些信号并夺回前台；
//! 工具子进程用 `setsid` 脱离控制终端，无法再抢走前台。

#[cfg(unix)]
use std::sync::Mutex;

#[cfg(unix)]
use libc::{c_int, sigaction, sighandler_t, SIGTSTP, SIGTTIN, SIGTTOU, SIG_IGN};

#[cfg(unix)]
struct SavedHandlers {
    ttin: sigaction,
    ttou: sigaction,
    tstp: sigaction,
}

#[cfg(unix)]
static SAVED: Mutex<Option<SavedHandlers>> = Mutex::new(None);

/// 【终端】【作业控制】忽略 SIGTTIN / SIGTTOU / SIGTSTP，并夺回前台进程组。
///
/// 重复调用只保存第一次的原处理函数。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
pub(crate) fn install() {
    #[cfg(unix)]
    install_unix();
}

/// 【终端】【作业控制】夺回前台后恢复原先的停止信号处理。
///
/// 未安装时为空操作。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
pub(crate) fn restore() {
    #[cfg(unix)]
    restore_unix();
}

/// 【终端】【作业控制】把当前进程组设为控制终端前台。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
pub(crate) fn claim_foreground() {
    #[cfg(unix)]
    claim_foreground_unix();
}

/// 【终端】【作业控制】让 tokio 子进程脱离控制终端。
///
/// 参数:
/// - `command`: 待启动命令
///
/// 返回:
/// - 无
pub(crate) fn isolate_tokio_command(command: &mut tokio::process::Command) {
    #[cfg(unix)]
    isolate_std_command(command.as_std_mut());
}

/// 【终端】【作业控制】让标准库子进程脱离控制终端。
///
/// 参数:
/// - `command`: 待启动命令
///
/// 返回:
/// - 无
#[cfg(unix)]
pub(crate) fn isolate_std_command(command: &mut std::process::Command) {
    use std::os::unix::process::CommandExt;
    unsafe {
        command.pre_exec(|| {
            // 1. 新会话没有控制终端，打开 /dev/tty 会得到 ENXIO
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

#[cfg(unix)]
fn install_unix() {
    // 1. 先忽略 SIGTTOU，否则后台写终端或 tcsetpgrp 会被内核停止
    let ttou = ignore_signal(SIGTTOU);
    claim_foreground_unix();
    let ttin = ignore_signal(SIGTTIN);
    let tstp = ignore_signal(SIGTSTP);
    if let Ok(mut guard) = SAVED.lock() {
        if guard.is_none() {
            *guard = Some(SavedHandlers { ttin, ttou, tstp });
        }
    }
}

#[cfg(unix)]
fn restore_unix() {
    claim_foreground_unix();
    let Ok(mut guard) = SAVED.lock() else {
        return;
    };
    let Some(saved) = guard.take() else {
        return;
    };
    restore_signal(SIGTTIN, saved.ttin);
    restore_signal(SIGTTOU, saved.ttou);
    restore_signal(SIGTSTP, saved.tstp);
}

#[cfg(unix)]
fn claim_foreground_unix() {
    // SAFETY: 只读取控制终端的前台进程组，失败时保持现状
    let tty = unsafe { libc::open(b"/dev/tty\0".as_ptr().cast(), libc::O_RDWR | libc::O_NOCTTY) };
    if tty < 0 {
        return;
    }
    let pgid = unsafe { libc::getpgrp() };
    let _ = unsafe { libc::tcsetpgrp(tty, pgid) };
    unsafe {
        libc::close(tty);
    }
}

#[cfg(unix)]
fn ignore_signal(signal: c_int) -> sigaction {
    let mut old = unsafe { std::mem::zeroed::<sigaction>() };
    let mut new = unsafe { std::mem::zeroed::<sigaction>() };
    new.sa_sigaction = SIG_IGN as sighandler_t;
    unsafe {
        libc::sigemptyset(&mut new.sa_mask);
        let _ = libc::sigaction(signal, &new, &mut old);
    }
    old
}

#[cfg(unix)]
fn restore_signal(signal: c_int, old: sigaction) {
    unsafe {
        let _ = libc::sigaction(signal, &old, std::ptr::null_mut());
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;
    use std::process::Stdio;
    use tokio::process::Command;

    /// 【终端】【作业控制】忽略 SIGTTIN 后对本进程发该信号，进程不得被停止。
    #[test]
    fn ignored_sigttin_does_not_stop_the_process() {
        install();
        unsafe {
            libc::raise(SIGTTIN);
        }
        restore();
    }

    /// 【终端】【作业控制】隔离后的子进程打不开控制终端。
    #[tokio::test]
    async fn isolated_child_cannot_open_controlling_tty() {
        let tty =
            unsafe { libc::open(b"/dev/tty\0".as_ptr().cast(), libc::O_RDWR | libc::O_NOCTTY) };
        if tty < 0 {
            return;
        }
        unsafe {
            libc::close(tty);
        }
        let mut command = Command::new("sh");
        command
            .args(["-c", "exec 3<>/dev/tty; echo attached"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        isolate_tokio_command(&mut command);
        let output = command.output().await.expect("spawn isolated child");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !output.status.success() || output.status.signal().is_some(),
            "isolated child must fail to open /dev/tty: {stderr}"
        );
        assert!(
            !String::from_utf8_lossy(&output.stdout).contains("attached"),
            "isolated child must not report an attached tty"
        );
    }
}
