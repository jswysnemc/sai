use anyhow::{ensure, Context, Result};
use std::ffi::c_void;
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, LocalFree, ERROR_INVALID_PARAMETER, HANDLE, WAIT_OBJECT_0,
};
use windows_sys::Win32::System::Threading::{
    OpenProcess, TerminateProcess, WaitForSingleObject, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_TERMINATE,
};
use windows_sys::Win32::UI::Shell::CommandLineToArgvW;

const SYNCHRONIZE: u32 = 0x00100000;

#[repr(C)]
#[derive(Clone, Copy)]
struct UnicodeString {
    length: u16,
    _maximum_length: u16,
    buffer: *const u16,
}

#[link(name = "ntdll")]
extern "system" {
    fn NtQueryInformationProcess(
        process: HANDLE,
        class: u32,
        information: *mut c_void,
        length: u32,
        returned: *mut u32,
    ) -> i32;
}

pub(in crate::plugins::compatibility::alarm_jobs) struct Handle(HANDLE);

impl Drop for Handle {
    /// 【旧闹钟兼容】【Windows 释放】关闭捕获的进程对象句柄。
    /// @returns 无
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

impl Handle {
    /// 【旧闹钟兼容】【Windows 存活】仅以进程对象信号确认退出，参数暂缺不等于进程结束。
    /// @returns 同一进程对象已经退出时为 true
    pub fn is_exited(&self) -> Result<bool> {
        let status = unsafe { WaitForSingleObject(self.0, 0) };
        ensure!(
            status != windows_sys::Win32::Foundation::WAIT_FAILED,
            "inspect legacy alarm process handle failed: {}",
            std::io::Error::last_os_error()
        );
        Ok(status == WAIT_OBJECT_0)
    }
    /// 【旧闹钟兼容】【Windows 句柄】打开可取消的进程对象，句柄不随 PID 复用改变。
    /// @param pid 待核验进程
    /// @returns 稳定句柄，目标不存在时返回 None
    pub fn open(pid: u32) -> Result<Option<Self>> {
        Self::with_access(
            pid,
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE | SYNCHRONIZE,
        )
    }

    /// 【旧闹钟兼容】【Windows 打开】读取与取消分别申请所需访问权。
    /// @param pid 目标；access 为访问掩码
    /// @returns 进程对象或 None
    fn with_access(pid: u32, access: u32) -> Result<Option<Self>> {
        let handle = unsafe { OpenProcess(access, 0, pid) };
        if handle.is_null() {
            let code = unsafe { GetLastError() };
            if code == ERROR_INVALID_PARAMETER {
                return Ok(None);
            }
            return Err(std::io::Error::from_raw_os_error(code as i32))
                .context("open legacy alarm process handle");
        }
        Ok(Some(Self(handle)))
    }

    /// 【旧闹钟兼容】【Windows 取消】只结束已核验的进程对象并等待退出。
    /// @returns 退出确认结果
    pub fn terminate(&self) -> Result<()> {
        if unsafe { WaitForSingleObject(self.0, 1000) } == WAIT_OBJECT_0 {
            return Ok(());
        }
        if unsafe { TerminateProcess(self.0, 1) } == 0 {
            if unsafe { WaitForSingleObject(self.0, 0) } != WAIT_OBJECT_0 {
                return Err(std::io::Error::last_os_error().into());
            }
        }
        ensure!(
            unsafe { WaitForSingleObject(self.0, 4000) } == WAIT_OBJECT_0,
            "legacy alarm worker did not exit after cancellation"
        );
        Ok(())
    }
}

/// 【旧闹钟兼容】【Windows 参数】读取进程对象的有界命令行，再按系统规则解析 argv。
/// @param pid 待观察 PID
/// @returns 参数列表，进程已退出时返回 None
pub(super) fn arguments(pid: u32) -> Result<Option<Vec<String>>> {
    let Some(handle) = Handle::with_access(pid, PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE)?
    else {
        return Ok(None);
    };
    if unsafe { WaitForSingleObject(handle.0, 0) } == WAIT_OBJECT_0 {
        return Ok(None);
    }
    let mut length = 0u32;
    unsafe {
        NtQueryInformationProcess(handle.0, 60, std::ptr::null_mut(), 0, &mut length);
    }
    ensure!(
        (std::mem::size_of::<UnicodeString>() as u32..=65536).contains(&length),
        "legacy alarm command line has an invalid byte length"
    );
    let mut bytes = vec![0u8; length as usize];
    let status = unsafe {
        NtQueryInformationProcess(handle.0, 60, bytes.as_mut_ptr().cast(), length, &mut length)
    };
    if status < 0 {
        if unsafe { WaitForSingleObject(handle.0, 0) } == WAIT_OBJECT_0 {
            return Ok(None);
        }
        anyhow::bail!("read legacy alarm command line failed: {status:#x}");
    }
    let string = unsafe { std::ptr::read_unaligned(bytes.as_ptr().cast::<UnicodeString>()) };
    let start = string.buffer as usize;
    let base = bytes.as_ptr() as usize;
    ensure!(
        string.length % 2 == 0
            && start % 2 == 0
            && start >= base
            && start
                .checked_add(string.length as usize)
                .is_some_and(|end| end <= base + bytes.len()),
        "legacy alarm command line points outside its buffer"
    );
    let mut wide =
        unsafe { std::slice::from_raw_parts(string.buffer, string.length as usize / 2) }.to_vec();
    wide.push(0);
    let mut count = 0;
    let argv = unsafe { CommandLineToArgvW(wide.as_ptr(), &mut count) };
    if argv.is_null() {
        return Err(std::io::Error::last_os_error().into());
    }
    let result = (|| {
        ensure!(
            (1..=32).contains(&count),
            "legacy alarm argv has too many entries"
        );
        let mut values = Vec::new();
        for index in 0..count as usize {
            let pointer = unsafe { *argv.add(index) };
            let mut end = 0;
            while end < 32768 && unsafe { *pointer.add(end) } != 0 {
                end += 1;
            }
            ensure!(end < 32768, "legacy alarm argument exceeds its byte limit");
            values.push(String::from_utf16(unsafe {
                std::slice::from_raw_parts(pointer, end)
            })?);
        }
        Ok(Some(values))
    })();
    unsafe {
        LocalFree(argv.cast());
    }
    result
}
