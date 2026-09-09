use anyhow::{Context, Result};
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::os::windows::process::CommandExt;
use std::process::ExitStatus;
use tokio::process::{Child, Command};
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows_sys::Win32::System::Threading::{
    OpenThread, ResumeThread, CREATE_SUSPENDED, THREAD_SUSPEND_RESUME,
};

/// 【插件进程】【Windows 归属】进程启动时保持挂起，加入独立 Job 后才允许执行。
pub(in crate::plugins::system) struct ProcessGroup {
    job: Option<OwnedHandle>,
}

impl ProcessGroup {
    /// 【插件进程】【创建 Job】关闭句柄时终止整个任务树，启动参数禁止初始线程提前执行。
    /// @param command 即将启动的命令
    /// @returns 尚未附加进程的任务守卫
    pub fn prepare(command: &mut Command) -> Result<Self> {
        // 【插件进程】【句柄归属】1. 创建匿名 Job，并立即交给唯一所有者管理
        let raw = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if raw.is_null() {
            return Err(std::io::Error::last_os_error()).context("create plugin process job");
        }
        let job = unsafe { OwnedHandle::from_raw_handle(raw) };
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let configured = unsafe {
            SetInformationJobObject(
                job.as_raw_handle(),
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if configured == 0 {
            return Err(std::io::Error::last_os_error()).context("configure plugin process job");
        }
        command.as_std_mut().creation_flags(CREATE_SUSPENDED);
        Ok(Self { job: Some(job) })
    }

    /// 【插件进程】【附加后恢复】先加入 Job，再恢复主线程，消除启动后代与归属绑定之间的竞态。
    /// @param child 保持初始线程挂起的进程
    /// @returns 归属绑定及恢复结果
    pub fn attach(&mut self, child: &Child) -> Result<()> {
        let job = self
            .job
            .as_ref()
            .context("plugin process job is unavailable")?;
        let process = child
            .raw_handle()
            .context("plugin process handle is unavailable")?;
        if unsafe { AssignProcessToJobObject(job.as_raw_handle(), process) } == 0 {
            return Err(std::io::Error::last_os_error()).context("attach plugin process to job");
        }
        resume_initial_thread(child.id().context("plugin process has no pid")?)
    }

    /// 【插件进程】【完整回收】等待主进程结束，再关闭 Job 清理仍存在的后代。
    /// @param child 当前调用拥有的进程
    /// @returns 原始进程退出状态
    pub async fn finish(&mut self, child: &mut Child) -> Result<ExitStatus> {
        let status = child.wait().await.context("wait for plugin process")?;
        self.job.take();
        Ok(status)
    }
}

/// 【插件进程】【主线程恢复】使用系统线程快照定位尚未执行的主线程，不依赖未公开的 NT 接口。
/// @param pid 刚创建的挂起进程
/// @returns 恢复成功；失败时调用方关闭 Job 终止进程
fn resume_initial_thread(pid: u32) -> Result<()> {
    let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if raw == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error()).context("snapshot plugin process threads");
    }
    let snapshot = unsafe { OwnedHandle::from_raw_handle(raw) };
    let mut entry: THREADENTRY32 = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
    let mut present = unsafe { Thread32First(snapshot.as_raw_handle(), &mut entry) };
    while present != 0 {
        if entry.th32OwnerProcessID == pid {
            let raw = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
            if raw.is_null() {
                return Err(std::io::Error::last_os_error()).context("open plugin initial thread");
            }
            let thread = unsafe { OwnedHandle::from_raw_handle(raw) };
            if unsafe { ResumeThread(thread.as_raw_handle()) } == u32::MAX {
                return Err(std::io::Error::last_os_error())
                    .context("resume plugin initial thread");
            }
            return Ok(());
        }
        entry.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
        present = unsafe { Thread32Next(snapshot.as_raw_handle(), &mut entry) };
    }
    anyhow::bail!("plugin initial thread was not found")
}
