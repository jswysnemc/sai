use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone, Debug)]
pub(crate) struct ProcessUsageSnapshot {
    pub pid: u32,
    pub uptime_seconds: u64,
    pub rss_bytes: Option<u64>,
    pub cpu_percent: f64,
}

#[derive(Clone)]
pub(crate) struct SystemMonitor {
    started_at: Instant,
    previous: Arc<Mutex<CpuSample>>,
}

#[derive(Clone, Copy)]
struct CpuSample {
    at: Instant,
    cpu_seconds: f64,
}

impl SystemMonitor {
    /// 创建进程用量采样器。
    ///
    /// 返回:
    /// - 系统用量采样器
    pub(crate) fn new() -> Self {
        let now = Instant::now();
        Self {
            started_at: now,
            previous: Arc::new(Mutex::new(CpuSample {
                at: now,
                cpu_seconds: process_cpu_seconds(),
            })),
        }
    }

    /// 读取当前进程用量并更新 CPU 采样基线。
    ///
    /// 返回:
    /// - 进程用量快照
    pub(crate) fn snapshot(&self) -> ProcessUsageSnapshot {
        let now = Instant::now();
        let cpu_seconds = process_cpu_seconds();
        let cpu_percent = self
            .previous
            .lock()
            .map(|mut previous| {
                let wall_seconds = now.duration_since(previous.at).as_secs_f64();
                let cpu_delta = (cpu_seconds - previous.cpu_seconds).max(0.0);
                *previous = CpuSample {
                    at: now,
                    cpu_seconds,
                };
                if wall_seconds > 0.0 {
                    cpu_delta / wall_seconds * 100.0
                } else {
                    0.0
                }
            })
            .unwrap_or_default();
        ProcessUsageSnapshot {
            pid: std::process::id(),
            uptime_seconds: now.duration_since(self.started_at).as_secs(),
            rss_bytes: process_rss_bytes(),
            cpu_percent,
        }
    }
}

/// 读取当前进程累计 CPU 秒数。
///
/// 返回:
/// - 用户态和内核态 CPU 秒数之和
#[cfg(unix)]
fn process_cpu_seconds() -> f64 {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    let status = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    if status != 0 {
        return 0.0;
    }
    let usage = unsafe { usage.assume_init() };
    timeval_seconds(usage.ru_utime) + timeval_seconds(usage.ru_stime)
}

/// 将 libc timeval 转换为秒。
///
/// 参数:
/// - `value`: timeval 值
///
/// 返回:
/// - 浮点秒数
#[cfg(unix)]
fn timeval_seconds(value: libc::timeval) -> f64 {
    value.tv_sec as f64 + value.tv_usec as f64 / 1_000_000.0
}

/// 读取 Windows 当前进程累计 CPU 秒数。
///
/// 返回:
/// - 用户态和内核态 CPU 秒数之和
#[cfg(windows)]
fn process_cpu_seconds() -> f64 {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};

    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    let status = unsafe {
        GetProcessTimes(
            GetCurrentProcess(),
            &mut creation,
            &mut exit,
            &mut kernel,
            &mut user,
        )
    };
    if status == 0 {
        return 0.0;
    }
    (filetime_ticks(kernel) + filetime_ticks(user)) as f64 / 10_000_000.0
}

/// 将 Windows FILETIME 转换为 100 纳秒计数。
///
/// 参数:
/// - `value`: Windows 文件时间
///
/// 返回:
/// - 100 纳秒计数
#[cfg(windows)]
fn filetime_ticks(value: windows_sys::Win32::Foundation::FILETIME) -> u64 {
    (u64::from(value.dwHighDateTime) << 32) | u64::from(value.dwLowDateTime)
}

/// 不支持进程 CPU 统计的平台返回零值。
///
/// 返回:
/// - 零
#[cfg(not(any(unix, windows)))]
fn process_cpu_seconds() -> f64 {
    0.0
}

/// 读取 Linux 当前进程常驻内存。
///
/// 返回:
/// - 常驻内存字节数，当前平台不可用时返回空值
#[cfg(target_os = "linux")]
fn process_rss_bytes() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("VmRSS:"))?;
    let kib = line.split_whitespace().nth(1)?.parse::<u64>().ok()?;
    Some(kib * 1024)
}

/// 读取 Windows 当前进程常驻内存。
///
/// 返回:
/// - 工作集字节数，读取失败时返回空值
#[cfg(windows)]
fn process_rss_bytes() -> Option<u64> {
    use windows_sys::Win32::System::ProcessStatus::{
        K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    let mut counters = PROCESS_MEMORY_COUNTERS {
        cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        ..Default::default()
    };
    let counter_size = counters.cb;
    let status =
        unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counter_size) };
    (status != 0).then_some(counters.WorkingSetSize as u64)
}

/// 读取 macOS 当前进程常驻内存。
///
/// 返回:
/// - 常驻内存字节数，读取失败时返回空值
#[cfg(target_os = "macos")]
fn process_rss_bytes() -> Option<u64> {
    let mut info = std::mem::MaybeUninit::<libc::rusage_info_v2>::zeroed();
    let mut buffer = info.as_mut_ptr().cast::<libc::c_void>();
    let status = unsafe {
        libc::proc_pid_rusage(
            std::process::id() as libc::c_int,
            libc::RUSAGE_INFO_V2,
            &mut buffer,
        )
    };
    if status != 0 {
        return None;
    }
    Some(unsafe { info.assume_init().ri_resident_size })
}

/// 其他平台暂不提供常驻内存。
///
/// 返回:
/// - 空值
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn process_rss_bytes() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshots_current_process_metrics() {
        let monitor = SystemMonitor::new();
        let snapshot = monitor.snapshot();
        assert_eq!(snapshot.pid, std::process::id());
        assert!(snapshot.cpu_percent >= 0.0);
        #[cfg(target_os = "linux")]
        assert!(snapshot.rss_bytes.unwrap_or_default() > 0);
        #[cfg(any(target_os = "macos", windows))]
        {
            // CI 沙箱可能无法读取进程内存计数，仅在有值时校验为正
            if let Some(rss) = snapshot.rss_bytes {
                assert!(rss > 0);
            }
        }
    }
}
