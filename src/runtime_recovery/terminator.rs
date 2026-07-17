use anyhow::Result;

/// 运行时进程终止器。
pub(crate) trait ProcessTerminator {
    /// 终止或强制结束运行时进程。
    ///
    /// 参数:
    /// - `pid`: 进程 ID
    /// - `pgid`: 进程组 ID
    /// - `force`: 是否强制结束
    ///
    /// 返回:
    /// - 进程是否已经停止
    fn terminate(&self, pid: u32, pgid: Option<i32>, force: bool) -> Result<bool>;
}

/// 平台进程终止器。
pub(crate) struct PlatformProcessTerminator;

impl ProcessTerminator for PlatformProcessTerminator {
    fn terminate(&self, pid: u32, pgid: Option<i32>, force: bool) -> Result<bool> {
        terminate_platform_process(pid, pgid, force)
    }
}

/// 终止平台进程或进程组。
///
/// 参数:
/// - `pid`: 进程 ID
/// - `pgid`: 进程组 ID
/// - `force`: 是否强制结束
///
/// 返回:
/// - 进程是否已经停止
fn terminate_platform_process(pid: u32, pgid: Option<i32>, force: bool) -> Result<bool> {
    #[cfg(unix)]
    {
        let signal = if force { libc::SIGKILL } else { libc::SIGTERM };
        let target = pgid.map(|pgid| -pgid).unwrap_or(pid as i32);
        unsafe {
            libc::kill(target, signal);
        }
        Ok(!platform_process_exists(pid))
    }
    #[cfg(windows)]
    {
        let mut command = std::process::Command::new("taskkill");
        command.arg("/PID").arg(pid.to_string()).arg("/T");
        if force {
            command.arg("/F");
        }
        let _ = command.output();
        Ok(!platform_process_exists(pid))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = pid;
        let _ = pgid;
        let _ = force;
        Ok(false)
    }
}

/// 判断平台进程是否仍存在。
///
/// 参数:
/// - `pid`: 进程 ID
///
/// 返回:
/// - 是否仍存在
fn platform_process_exists(pid: u32) -> bool {
    #[cfg(unix)]
    {
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }
    #[cfg(windows)]
    {
        std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()))
            .unwrap_or(false)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = pid;
        false
    }
}
