/// 【会话在线】【进程查询】通过操作系统接口判断进程是否仍存活，不启动外部命令。
/// @param pid 待查询的进程标识
/// @returns 进程存活或因权限无法确认时返回 true，已退出或标识无效时返回 false
pub(super) fn process_exists(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    if pid == std::process::id() {
        return true;
    }
    #[cfg(unix)]
    {
        if pid > i32::MAX as u32 {
            return false;
        }
        let status = unsafe { libc::kill(pid as i32, 0) };
        status == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{
            CloseHandle, GetLastError, ERROR_ACCESS_DENIED, STILL_ACTIVE,
        };
        use windows_sys::Win32::System::Threading::{
            GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };
        // 1. 【会话在线】【原生查询】每条在线记录只打开一个轻量进程句柄，避免逐会话启动 tasklist
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if handle.is_null() {
            return unsafe { GetLastError() } == ERROR_ACCESS_DENIED;
        }
        let mut code = 0;
        let succeeded = unsafe { GetExitCodeProcess(handle, &mut code) } != 0;
        unsafe { CloseHandle(handle) };
        // 2. 【会话在线】【权限保护】查询失败不把可能仍存活的进程当作已退出
        !succeeded || code == STILL_ACTIVE as u32
    }
    #[cfg(not(any(unix, windows)))]
    {
        false
    }
}
