#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;
#[cfg(target_os = "linux")]
use linux as native;
#[cfg(target_os = "macos")]
use macos as native;
#[cfg(windows)]
use windows as native;

use super::record::LegacyRecord;
use anyhow::{ensure, Result};
use std::{collections::BTreeMap, path::Path};

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
pub(super) use native::Handle;

/// 【旧闹钟兼容】【完整身份】核对旧入口及全部业务参数，状态目录必须指向同一个真实目录。
/// @param arguments 进程 argv；record 为旧记录；state_dir 为可信状态目录
/// @returns 是否属于这条旧闹钟
pub(super) fn matches(arguments: &[String], record: &LegacyRecord, state_dir: &Path) -> bool {
    let expected = if record.audio_file.is_some() { 12 } else { 10 };
    if arguments.len() != expected
        || arguments
            .get(1)
            .is_none_or(|value| value != "__alarm-worker")
    {
        return false;
    }
    let mut options = BTreeMap::new();
    for pair in arguments[2..].chunks_exact(2) {
        if options.insert(pair[0].as_str(), pair[1].as_str()).is_some() {
            return false;
        }
    }
    if options.get("--id") != Some(&record.id.as_str())
        || options.get("--time") != Some(&record.time.as_str())
        || options.get("--label") != Some(&record.label.as_str())
    {
        return false;
    }
    let Some(directory) = options.get("--state-dir") else {
        return false;
    };
    match (
        dunce::canonicalize(directory),
        dunce::canonicalize(state_dir),
    ) {
        (Ok(actual), Ok(expected)) if actual == expected => {}
        _ => return false,
    }
    match (&record.audio_file, options.get("--audio-file")) {
        (None, None) => true,
        (Some(path), Some(argument)) => path == Path::new(argument),
        _ => false,
    }
}

/// 【旧闹钟兼容】【进程观察】查询只检查身份，不发送信号或写入旧记录。
/// @param record 旧记录；state_dir 为状态目录
/// @returns 原工作进程仍具有匹配身份时为 true
pub(super) fn is_worker(record: &LegacyRecord, state_dir: &Path) -> Result<bool> {
    let Some(pid) = record.pid else {
        return Ok(true);
    };
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    {
        Ok(native::arguments(pid)?.is_some_and(|arguments| matches(&arguments, record, state_dir)))
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        let _ = (pid, state_dir);
        anyhow::bail!("legacy alarm process inspection is unavailable on this platform")
    }
}

/// 【旧闹钟兼容】【取消句柄】先持有稳定进程句柄再核对身份，之后不会按可复用 PID 发送信号。
/// @param record 旧记录；state_dir 为可信目录
/// @returns 已核验句柄；进程已经退出时返回 None
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
pub(super) fn capture(record: &LegacyRecord, state_dir: &Path) -> Result<Option<Handle>> {
    let Some(pid) = record.pid else {
        return Ok(None);
    };
    ensure!(
        pid != std::process::id(),
        "cannot cancel the current process as a legacy alarm"
    );
    let Some(handle) = Handle::open(pid)? else {
        return Ok(None);
    };
    verify(handle, pid, record, state_dir, native::arguments)
}

/// 【旧闹钟兼容】【退出复核】参数缺失不能证明退出，只有稳定句柄确认结束后才能返回 None。
/// @param handle 稳定句柄；pid 为观察标识；record 为旧记录；state_dir 为可信目录；inspect 为参数读取边界
/// @returns 核验句柄或确认退出；参数暂缺但进程仍存活时要求重试
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
fn verify(
    handle: Handle,
    pid: u32,
    record: &LegacyRecord,
    state_dir: &Path,
    mut inspect: impl FnMut(u32) -> Result<Option<Vec<String>>>,
) -> Result<Option<Handle>> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(250);
    loop {
        if handle.is_exited()? {
            return Ok(None);
        }
        let arguments = inspect(pid)?;
        if handle.is_exited()? {
            return Ok(None);
        }
        if let Some(arguments) = arguments {
            ensure!(
                matches(&arguments, record, state_dir),
                "legacy alarm process identity does not match; no signal was sent"
            );
            return Ok(Some(handle));
        }
        ensure!(
            std::time::Instant::now() < deadline,
            "legacy alarm process arguments are temporarily unavailable; retry cancellation"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "../tests/capture.rs"]
mod tests;

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
pub(super) struct Handle;

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
impl Handle {
    /// 【旧闹钟兼容】【平台边界】没有稳定句柄支持的平台不能按 PID 降级取消。
    /// @returns 固定拒绝
    pub fn terminate(&self) -> Result<()> {
        anyhow::bail!("stable legacy alarm cancellation is unavailable on this platform")
    }
}

/// 【旧闹钟兼容】【平台边界】未知平台只允许取消没有工作进程的旧记录。
/// @param record 旧任务；state_dir 为状态目录
/// @returns 无进程时返回 None，否则明确拒绝
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
pub(super) fn capture(record: &LegacyRecord, _state_dir: &Path) -> Result<Option<Handle>> {
    ensure!(
        record.pid.is_none(),
        "stable legacy alarm cancellation is unavailable on this platform"
    );
    Ok(None)
}
