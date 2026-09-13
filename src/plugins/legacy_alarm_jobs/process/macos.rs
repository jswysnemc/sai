use anyhow::{ensure, Result};

extern "C" {
    static mach_task_self_: u32;
    fn task_for_pid(target: u32, pid: i32, task: *mut u32) -> i32;
    fn task_terminate(task: u32) -> i32;
    fn mach_port_deallocate(task: u32, name: u32) -> i32;
    fn mach_port_type(task: u32, name: u32, kind: *mut u32) -> i32;
}

pub(in crate::plugins::legacy_alarm_jobs) struct Handle(u32);

impl Drop for Handle {
    /// 【旧闹钟兼容】【macOS 释放】释放任务端口，不保留跨任务引用。
    /// @returns 无
    fn drop(&mut self) {
        unsafe {
            mach_port_deallocate(mach_task_self_, self.0);
        }
    }
}

impl Handle {
    /// 【旧闹钟兼容】【macOS 存活】仅以稳定任务端口确认退出，参数暂缺不代表结束。
    /// @returns 原任务端口已经失效时为 true
    pub fn is_exited(&self) -> Result<bool> {
        self.wait(0)
    }
    /// 【旧闹钟兼容】【macOS 句柄】取得稳定任务端口，系统拒绝时不降级为裸 PID 信号。
    /// @param pid 待核验进程
    /// @returns 任务端口，目标已退出时返回 None
    pub fn open(pid: u32) -> Result<Option<Self>> {
        ensure!(pid <= i32::MAX as u32, "invalid legacy alarm PID");
        let mut task = 0;
        let status = unsafe { task_for_pid(mach_task_self_, pid as i32, &mut task) };
        if status != 0 {
            if arguments(pid)?.is_none() {
                return Ok(None);
            }
            anyhow::bail!("macOS denied a stable legacy alarm task handle ({status}); worker was not signalled");
        }
        ensure!(
            task != 0,
            "macOS returned an empty legacy alarm task handle"
        );
        Ok(Some(Self(task)))
    }

    /// 【旧闹钟兼容】【macOS 取消】终止已核验的任务端口，不重新按 PID 选择目标。
    /// @returns 内核终止结果
    pub fn terminate(&self) -> Result<()> {
        if self.wait(1000)? {
            return Ok(());
        }
        let status = unsafe { task_terminate(self.0) };
        ensure!(
            status == 0 || self.wait(0)?,
            "macOS could not terminate the legacy alarm task ({status})"
        );
        ensure!(
            self.wait(2000)?,
            "legacy alarm worker did not exit after cancellation"
        );
        Ok(())
    }

    /// 【旧闹钟兼容】【macOS 退出确认】以任务端口变为失效名字确认退出，不再根据 PID 判断。
    /// @param timeout_ms 等待毫秒数
    /// @returns 时限内退出时为 true
    fn wait(&self, timeout_ms: u64) -> Result<bool> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
        loop {
            let mut kind = 0;
            let status = unsafe { mach_port_type(mach_task_self_, self.0, &mut kind) };
            ensure!(
                status == 0,
                "inspect legacy alarm task port failed ({status})"
            );
            if kind & 0x00100000 != 0 {
                return Ok(true);
            }
            if std::time::Instant::now() >= deadline {
                return Ok(false);
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
}

/// 【旧闹钟兼容】【macOS 参数】有界读取内核 argv，保留标签中的空白和空参数。
/// @param pid 待观察 PID
/// @returns 参数列表，进程已退出时返回 None
pub(super) fn arguments(pid: u32) -> Result<Option<Vec<String>>> {
    ensure!(pid <= i32::MAX as u32, "invalid legacy alarm PID");
    let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid as i32];
    let mut bytes = vec![0u8; 65536];
    let mut length = bytes.len();
    let status = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            mib.len() as u32,
            bytes.as_mut_ptr().cast(),
            &mut length,
            std::ptr::null_mut(),
            0,
        )
    };
    if status < 0 {
        let error = std::io::Error::last_os_error();
        if matches!(error.raw_os_error(), Some(libc::ESRCH | libc::ENOENT)) {
            return Ok(None);
        }
        return Err(error.into());
    }
    ensure!(
        (4..=65536).contains(&length),
        "legacy alarm argv has an invalid byte length"
    );
    bytes.truncate(length);
    let count = i32::from_ne_bytes(bytes[..4].try_into()?);
    ensure!(
        (1..=32).contains(&count),
        "legacy alarm argv has too many entries"
    );
    let mut offset = 4;
    while offset < bytes.len() && bytes[offset] != 0 {
        offset += 1;
    }
    while offset < bytes.len() && bytes[offset] == 0 {
        offset += 1;
    }
    let mut values = Vec::new();
    for _ in 0..count {
        let tail = bytes
            .get(offset..)
            .ok_or_else(|| anyhow::anyhow!("truncated legacy alarm argv"))?;
        let end = tail
            .iter()
            .position(|byte| *byte == 0)
            .ok_or_else(|| anyhow::anyhow!("unterminated legacy alarm argument"))?;
        values.push(std::str::from_utf8(&tail[..end])?.to_string());
        offset += end + 1;
    }
    Ok(Some(values))
}
