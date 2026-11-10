use super::identity::{wide, Capabilities, Identity};
use anyhow::{Context, Result};
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::Path;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Security::SECURITY_CAPABILITIES;
use windows_sys::Win32::System::JobObjects::*;
use windows_sys::Win32::System::Threading::*;
use windows_sys::Win32::System::WindowsProgramming::PROCESS_CREATION_ALL_APPLICATION_PACKAGES_OPT_OUT;

/// 【Windows沙箱】【启动属性】对齐分配 Win32 属性列表，并负责销毁。
struct Attributes {
    buffer: Vec<usize>,
}
impl Attributes {
    /// 【Windows沙箱】【启动属性创建】按属性数量分配列表；参数为数量，返回列表或系统错误。
    fn new(count: u32) -> Result<Self> {
        let mut size = 0;
        unsafe {
            InitializeProcThreadAttributeList(std::ptr::null_mut(), count, 0, &mut size);
        }
        let mut this = Self {
            buffer: vec![0; size.div_ceil(std::mem::size_of::<usize>())],
        };
        anyhow::ensure!(
            unsafe { InitializeProcThreadAttributeList(this.ptr(), count, 0, &mut size) } != 0,
            "initialize process attributes: {}",
            std::io::Error::last_os_error()
        );
        Ok(this)
    }
    /// 【Windows沙箱】【属性指针】返回稳定的属性存储指针；无参数，返回 Win32 指针。
    fn ptr(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.buffer.as_mut_ptr().cast()
    }
    /// 【Windows沙箱】【属性写入】绑定仍由调用者持有的属性值；参数为属性键和可变值，返回结果。
    fn set<T>(&mut self, key: u32, value: &mut T) -> Result<()> {
        anyhow::ensure!(
            unsafe {
                UpdateProcThreadAttribute(
                    self.ptr(),
                    0,
                    key as usize,
                    (value as *mut T).cast(),
                    std::mem::size_of::<T>(),
                    std::ptr::null_mut(),
                    std::ptr::null(),
                )
            } != 0,
            "set sandbox process attribute {key}: {}",
            std::io::Error::last_os_error()
        );
        Ok(())
    }
}
impl Drop for Attributes {
    /// 【Windows沙箱】【属性释放】释放 Win32 属性列表；无参数、无返回值。
    fn drop(&mut self) {
        unsafe {
            DeleteProcThreadAttributeList(self.ptr());
        }
    }
}

/// 【Windows沙箱】【受限启动】在执行任何用户指令前绑定 LPAC 和关闭即清理的 Job。
/// 参数：容器身份、能力、Shell、参数和目录；返回真实进程退出码。
pub(super) fn run(
    identity: Option<&Identity>,
    capabilities: &mut Capabilities,
    shell: &Path,
    args: &[std::ffi::OsString],
    cwd: &Path,
) -> Result<u32> {
    let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    anyhow::ensure!(
        !job.is_null(),
        "create sandbox job: {}",
        std::io::Error::last_os_error()
    );
    let job = unsafe { OwnedHandle::from_raw_handle(job) };
    let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    anyhow::ensure!(
        unsafe {
            SetInformationJobObject(
                job.as_raw_handle(),
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&limits) as u32,
            )
        } != 0,
        "configure sandbox job: {}",
        std::io::Error::last_os_error()
    );
    let mut security = SECURITY_CAPABILITIES {
        AppContainerSid: identity.map_or(std::ptr::null_mut(), |identity| identity.sid),
        Capabilities: capabilities.entries.as_mut_ptr(),
        CapabilityCount: capabilities.entries.len() as u32,
        Reserved: 0,
    };
    let mut opt_out = PROCESS_CREATION_ALL_APPLICATION_PACKAGES_OPT_OUT;
    let mut jobs = [job.as_raw_handle()];
    let stdio = super::handles::duplicate_stdio()?;
    let mut inherited = stdio.each_ref().map(AsRawHandle::as_raw_handle);
    let mut attributes = Attributes::new(if identity.is_some() { 4 } else { 2 })?;
    attributes.set(PROC_THREAD_ATTRIBUTE_HANDLE_LIST, &mut inherited)?;
    if identity.is_some() {
        attributes.set(PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES, &mut security)?;
        attributes.set(
            PROC_THREAD_ATTRIBUTE_ALL_APPLICATION_PACKAGES_POLICY,
            &mut opt_out,
        )?;
    }
    attributes.set(PROC_THREAD_ATTRIBUTE_JOB_LIST, &mut jobs)?;
    let mut startup: STARTUPINFOEXW = unsafe { std::mem::zeroed() };
    startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = inherited[0];
    startup.StartupInfo.hStdOutput = inherited[1];
    startup.StartupInfo.hStdError = inherited[2];
    startup.lpAttributeList = attributes.ptr();
    let shell_text = shell.to_string_lossy();
    let shell_text = shell_text.strip_prefix(r"\\?\").unwrap_or(&shell_text);
    let application = wide(shell_text);
    let command_line = if crate::platform::shell_selection::shell_flavor(shell.as_os_str())
        == crate::platform::shell_selection::ShellFlavor::Cmd
    {
        format!(
            "{} /D /S /C \"{}\"",
            quote(shell_text),
            args.last().context("cmd script missing")?.to_string_lossy()
        )
    } else {
        std::iter::once(shell_text.to_string())
            .chain(args.iter().map(|arg| arg.to_string_lossy().into_owned()))
            .map(|arg| quote(&arg))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let mut command = wide(&command_line);
    let cwd_text = cwd.to_string_lossy();
    let cwd_text = cwd_text.strip_prefix(r"\\?\").unwrap_or(&cwd_text);
    let cwd = wide(cwd_text);
    let mut info: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
    anyhow::ensure!(
        unsafe {
            CreateProcessW(
                application.as_ptr(),
                command.as_mut_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
                EXTENDED_STARTUPINFO_PRESENT | CREATE_SUSPENDED,
                std::ptr::null(),
                cwd.as_ptr(),
                &startup.StartupInfo,
                &mut info,
            )
        } != 0,
        "launch AppContainer shell: {}",
        std::io::Error::last_os_error()
    );
    let process = unsafe { OwnedHandle::from_raw_handle(info.hProcess) };
    let thread = unsafe { OwnedHandle::from_raw_handle(info.hThread) };
    anyhow::ensure!(
        unsafe { ResumeThread(thread.as_raw_handle()) } != u32::MAX,
        "resume sandbox process failed"
    );
    anyhow::ensure!(
        unsafe { WaitForSingleObject(process.as_raw_handle(), INFINITE) } == WAIT_OBJECT_0,
        "wait for sandbox process failed"
    );
    let mut exit = 0;
    if unsafe { GetExitCodeProcess(process.as_raw_handle(), &mut exit) } == 0 {
        return Err(std::io::Error::last_os_error()).context("sandbox exit code");
    }
    // 1. 【Windows沙箱】【后代清理】回收授权之前必须先关闭 Job，终止所有存活后代
    drop(job);
    Ok(exit)
}

/// 【Windows沙箱】【参数引用】按 Windows CRT 规则转义参数，避免引号和末尾反斜杠改变含义。
/// 参数：单个参数；返回带引号参数。
fn quote(value: &str) -> String {
    let mut output = String::from("\"");
    let mut slashes = 0;
    for ch in value.chars() {
        if ch == '\\' {
            slashes += 1;
            continue;
        }
        if ch == '"' {
            output.push_str(&"\\".repeat(slashes * 2 + 1));
        } else {
            output.push_str(&"\\".repeat(slashes));
        }
        output.push(ch);
        slashes = 0;
    }
    output.push_str(&"\\".repeat(slashes * 2));
    output.push('"');
    output
}
