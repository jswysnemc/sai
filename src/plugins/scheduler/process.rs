use super::record::JobRecord;
use anyhow::{ensure, Context, Result};
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use std::process::{Child, Command, Stdio};

/// 【插件调度】【执行锁】独立工作进程持有整个执行期间的锁，PID 用于显示和启动握手。
pub(super) struct ExecutionLease(std::fs::File);

impl Drop for ExecutionLease {
    /// 【插件调度】【锁释放】显式释放，避免依赖继承文件描述符的关闭顺序。
    /// @returns 无
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

/// 【插件调度】【独占执行】同一任务只能由一个工作进程占有，不终止任何 PID。
/// @param directory 私有目录；id 为合法任务标识
/// @returns 独占守卫；其他进程占有时返回 None
pub(super) fn claim(directory: &Dir, id: &str) -> Result<Option<ExecutionLease>> {
    sai_plugin_runtime::host::validate_scheduled_id(id)?;
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .follow(FollowSymlinks::No);
    #[cfg(unix)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = directory
        .open_with(format!("{id}.lock"), &options)?
        .into_std();
    ensure!(
        file.metadata()?.is_file(),
        "scheduled execution lock requires a regular file"
    );
    match file.try_lock() {
        Ok(()) => Ok(Some(ExecutionLease(file))),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(error)) => Err(error.into()),
    }
}

/// 【插件调度】【启动边界】测试可以替换进程创建，正式实现只启动当前 Sai 程序。
pub(super) trait WorkerLauncher: Send + Sync {
    /// 【插件调度】【创建进程】创建尚未发布的工作进程，失败不能留下无记录任务。
    /// @param record 已验证的宿主记录
    /// @returns 发布前守卫
    fn spawn(&self, record: &JobRecord) -> Result<Box<dyn WorkerHandle>>;
}

pub(super) trait WorkerHandle {
    /// 【插件调度】【进程标识】仅用于记录与启动握手，不用于取消信号。
    /// @returns 子进程标识
    fn id(&self) -> u32;
    /// 【插件调度】【发布确认】持久记录完成后允许后台进程存续，并安排退出回收。
    /// @returns 无
    fn commit(self: Box<Self>);
}

pub(super) struct SystemLauncher;
struct PendingWorker(Option<Child>);

impl Drop for PendingWorker {
    /// 【插件调度】【启动回滚】记录发布失败时只终止自己持有的子进程句柄。
    /// @returns 无
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl WorkerHandle for PendingWorker {
    /// 【插件调度】【进程标识】读取尚未交给退出回收线程的子进程标识。
    /// @returns PID
    fn id(&self) -> u32 {
        self.0.as_ref().expect("pending child").id()
    }

    /// 【插件调度】【进程回收】父进程存续时回收退出状态，父进程退出不取消任务。
    /// @returns 无
    fn commit(mut self: Box<Self>) {
        let mut child = self.0.take().expect("pending child");
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
}

impl WorkerLauncher for SystemLauncher {
    /// 【插件调度】【固定工作程序】参数按独立 argv 传递，工作目录来自可信调用。
    /// @param record 待启动记录
    /// @returns 启动守卫；没有 shell 拼接或用户指定程序
    fn spawn(&self, record: &JobRecord) -> Result<Box<dyn WorkerHandle>> {
        let mut command = Command::new(std::env::current_exe()?);
        command
            .arg("__plugin-job-worker")
            .arg("--state-dir")
            .arg(&record.paths.state_dir)
            .arg("--plugin")
            .arg(&record.plugin)
            .arg("--id")
            .arg(&record.task.id)
            .arg("--launch")
            .arg(&record.launch)
            .current_dir(&record.workdir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        // 1. 【插件调度】【会话分离】工作进程不接收父终端进程组的交互取消信号
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            unsafe {
                command.pre_exec(|| {
                    if libc::setsid() == -1 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            use windows_sys::Win32::System::Threading::{
                CREATE_NEW_PROCESS_GROUP, DETACHED_PROCESS,
            };
            command.creation_flags(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS);
        }
        // 2. 【插件调度】【发布守卫】持久记录写入失败时，守卫负责结束这个子进程
        Ok(Box::new(PendingWorker(Some(
            command.spawn().context("start plugin scheduled worker")?,
        ))))
    }
}
