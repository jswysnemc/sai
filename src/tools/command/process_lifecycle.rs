use anyhow::Result;

/// 【命令】【执行收尾】前台执行的进程树和输出读取任务共用生命周期
pub(super) struct CommandTreeGuard {
    pid: u32,
    readers: Vec<tokio::task::AbortHandle>,
    #[cfg(windows)]
    job: Option<std::os::windows::io::OwnedHandle>,
    killed: bool,
}

impl CommandTreeGuard {
    /// 【命令】【进程绑定】绑定独立进程组或 Windows Job，失败时拒绝继续执行
    /// 参数：`pid` 为刚创建的进程；返回生命周期守卫
    pub(super) fn new(pid: u32) -> Result<Self> {
        Ok(Self {
            pid,
            readers: Vec::new(),
            killed: false,
            #[cfg(windows)]
            job: Some(super::windows_job::attach(pid)?),
        })
    }

    /// 【命令】【管道绑定】记录异步读取任务，取消时一起释放管道
    /// 参数：任务取消句柄；返回无
    pub(super) fn track_reader(&mut self, reader: tokio::task::AbortHandle) {
        self.readers.push(reader);
    }

    /// 【命令】【进程树终止】取消、超时和 Shell 退出均清理后代进程
    /// 参数：无；返回无
    pub(super) fn terminate(&mut self) {
        if self.killed {
            return;
        }
        self.killed = true;
        #[cfg(unix)]
        unsafe {
            // 1. 【命令】【进程组终止】Shell 已通过 setsid 建立独立进程组
            libc::kill(-(self.pid as i32), libc::SIGKILL);
        }
        #[cfg(windows)]
        self.job.take();
    }
}

impl Drop for CommandTreeGuard {
    /// 【命令】【取消收尾】执行 future 被丢弃时也终止整棵进程树；无参数、无返回值
    fn drop(&mut self) {
        self.terminate();
        for task in &self.readers {
            task.abort();
        }
    }
}
