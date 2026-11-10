use anyhow::{Context, Result};
use std::os::windows::io::{FromRawHandle, OwnedHandle};
use windows_sys::Win32::Foundation::{
    DuplicateHandle, DUPLICATE_SAME_ACCESS, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::System::Console::{
    GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
};
use windows_sys::Win32::System::Threading::GetCurrentProcess;

/// 【Windows沙箱】【管道继承】只复制三个标准句柄，不把启动器的文件、锁和 Job 交给子进程。
/// 参数：无；返回在进程创建期间持有的可继承句柄。
pub(super) fn duplicate_stdio() -> Result<[OwnedHandle; 3]> {
    let mut handles = Vec::new();
    for kind in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
        let source = unsafe { GetStdHandle(kind) };
        anyhow::ensure!(
            !source.is_null() && source != INVALID_HANDLE_VALUE,
            "missing standard handle"
        );
        let process = unsafe { GetCurrentProcess() };
        let mut target = std::ptr::null_mut();
        if unsafe {
            DuplicateHandle(
                process,
                source,
                process,
                &mut target,
                0,
                1,
                DUPLICATE_SAME_ACCESS,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error())
                .context("duplicate sandbox standard handle");
        }
        handles.push(unsafe { OwnedHandle::from_raw_handle(target) });
    }
    Ok(handles.try_into().expect("three standard handles"))
}
