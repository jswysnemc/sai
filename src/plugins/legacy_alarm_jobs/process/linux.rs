use anyhow::{ensure, Context, Result};
use std::{
    io::Read,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
};

pub(in crate::plugins::legacy_alarm_jobs) struct Handle(OwnedFd);

impl Handle {
    /// 【旧闹钟兼容】【Linux 存活】进程参数尚未发布时，也只能通过内核句柄确认退出。
    /// @returns 同一进程实例已经退出时为 true
    pub fn is_exited(&self) -> Result<bool> {
        self.wait(0)
    }
    /// 【旧闹钟兼容】【Linux 句柄】pidfd 绑定内核进程实例，PID 复用不会改变目标。
    /// @param pid 持久记录中的待核验 PID
    /// @returns 稳定句柄，目标不存在时返回 None
    pub fn open(pid: u32) -> Result<Option<Self>> {
        let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
        if fd < 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::ESRCH) {
                return Ok(None);
            }
            return Err(error).context("open stable legacy alarm process handle");
        }
        Ok(Some(Self(unsafe { OwnedFd::from_raw_fd(fd as i32) })))
    }

    /// 【旧闹钟兼容】【Linux 取消】通过已核验句柄请求退出，超时后只强制结束同一个进程实例。
    /// @returns 目标确认退出时成功
    pub fn terminate(&self) -> Result<()> {
        if self.wait(1000)? {
            return Ok(());
        }
        self.signal(libc::SIGTERM)?;
        if self.wait(2000)? {
            return Ok(());
        }
        self.signal(libc::SIGKILL)?;
        ensure!(
            self.wait(2000)?,
            "legacy alarm worker did not exit after cancellation"
        );
        Ok(())
    }

    /// 【旧闹钟兼容】【Linux 信号】所有信号均发送到 pidfd，退出中的目标按已结束处理。
    /// @param signal 信号编号
    /// @returns 发送结果
    fn signal(&self, signal: i32) -> Result<()> {
        let result = unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                self.0.as_raw_fd(),
                signal,
                std::ptr::null::<libc::siginfo_t>(),
                0,
            )
        };
        if result < 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::ESRCH) {
                return Err(error.into());
            }
        }
        Ok(())
    }

    /// 【旧闹钟兼容】【Linux 退出确认】等待句柄可读，不向其他 PID 查询或发送信号。
    /// @param timeout_ms 等待毫秒数
    /// @returns 时限内退出时为 true
    fn wait(&self, timeout_ms: i32) -> Result<bool> {
        let deadline =
            std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms as u64);
        loop {
            let remaining = deadline
                .saturating_duration_since(std::time::Instant::now())
                .as_millis() as i32;
            let mut poll = libc::pollfd {
                fd: self.0.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            let result = unsafe { libc::poll(&mut poll, 1, remaining) };
            if result < 0 {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() == Some(libc::EINTR) {
                    if std::time::Instant::now() >= deadline {
                        return Ok(false);
                    }
                    continue;
                }
                return Err(error.into());
            }
            ensure!(
                poll.revents & (libc::POLLERR | libc::POLLNVAL) == 0,
                "legacy alarm process handle became invalid"
            );
            return Ok(result > 0 && poll.revents & libc::POLLIN != 0);
        }
    }
}

/// 【旧闹钟兼容】【Linux 参数】有界读取进程参数，启动中的空参数短暂重试，退出后返回缺失。
/// @param pid 待观察 PID
/// @returns UTF-8 argv 或 None
pub(super) fn arguments(pid: u32) -> Result<Option<Vec<String>>> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(250);
    loop {
        let Some(mut bytes) = read_proc(pid, "cmdline", 65536)? else {
            return Ok(None);
        };
        if !bytes.is_empty() {
            if bytes.last() == Some(&0) {
                bytes.pop();
            }
            let arguments = bytes
                .split(|byte| *byte == 0)
                .map(|value| Ok(std::str::from_utf8(value)?.to_string()))
                .collect::<Result<Vec<_>>>()?;
            return Ok(Some(arguments));
        }
        // 1. 【旧闹钟兼容】【启动窗口】exec 可能先关闭启动管道、随后才发布 argv，空参数不能代表退出
        let Some(stat) = read_proc(pid, "stat", 4096)? else {
            return Ok(None);
        };
        let end = stat
            .iter()
            .rposition(|byte| *byte == b')')
            .context("invalid legacy process state")?;
        if matches!(stat.get(end + 2), Some(b'Z' | b'X' | b'x')) {
            return Ok(None);
        }
        ensure!(
            std::time::Instant::now() < deadline,
            "legacy alarm process arguments are temporarily unavailable; retry inspection"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// 【旧闹钟兼容】【Linux 有界读取】读取固定 proc 字段，目标退出与其他读取错误分别处理。
/// @param pid 目标进程；field 为内部固定字段；limit 为字节上限
/// @returns 完整字节，进程退出时为 None
fn read_proc(pid: u32, field: &str, limit: usize) -> Result<Option<Vec<u8>>> {
    let file = match std::fs::File::open(format!("/proc/{pid}/{field}")) {
        Ok(file) => file,
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                || error.raw_os_error() == Some(libc::ESRCH) =>
        {
            return Ok(None)
        }
        Err(error) => return Err(error.into()),
    };
    let mut bytes = Vec::new();
    match file.take(limit as u64 + 1).read_to_end(&mut bytes) {
        Err(error) if error.raw_os_error() == Some(libc::ESRCH) => return Ok(None),
        result => {
            result?;
        }
    }
    ensure!(
        bytes.len() <= limit,
        "legacy alarm process field exceeds its byte limit"
    );
    Ok(Some(bytes))
}
