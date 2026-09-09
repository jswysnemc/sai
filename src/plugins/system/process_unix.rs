use anyhow::{Context, Result};
use std::os::unix::process::CommandExt;
use std::process::ExitStatus;
use std::time::Duration;
use tokio::process::{Child, Command};

/// 【插件进程】【Unix 归属】保留进程组直到完整调用结束，取消时终止组内后代。
pub(in crate::plugins::system) struct ProcessGroup {
    pid: Option<i32>,
}

impl ProcessGroup {
    /// 【插件进程】【创建进程组】在 exec 前建立独立进程组。
    /// @param command 即将启动的命令
    /// @returns 尚未绑定 PID 的守卫
    pub fn prepare(command: &mut Command) -> Result<Self> {
        command.as_std_mut().process_group(0);
        Ok(Self { pid: None })
    }

    /// 【插件进程】【绑定进程组】记录仍由 Child 句柄持有的组长 PID。
    /// @param child 已启动且尚未等待的进程
    /// @returns 绑定结果
    pub fn attach(&mut self, child: &Child) -> Result<()> {
        self.pid = Some(i32::try_from(
            child.id().context("plugin process has no pid")?,
        )?);
        Ok(())
    }

    /// 【插件进程】【完整回收】先观察退出而不回收 PID，再终止后代，最后等待组长回收。
    /// @param child 当前调用拥有的子进程
    /// @returns 原始进程退出状态
    pub async fn finish(&mut self, child: &mut Child) -> Result<ExitStatus> {
        let pid = self.pid.context("plugin process group is not attached")?;
        while !exited_without_reaping(pid)? {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        // 【插件进程】【PID 生命周期】1. 组长尚未回收，组号不能重用为其他调用的进程组
        self.terminate();
        child.wait().await.context("reap plugin process")
    }

    /// 【插件进程】【终止进程组】终止本次调用的进程及后代，只执行一次。
    /// @returns 无；后续由 Child 等待或 Tokio 回收退出进程
    fn terminate(&mut self) {
        if let Some(pid) = self.pid.take() {
            // 【插件进程】【归属检查】PID 来自仍未回收的 Child，负值只指向该调用创建的进程组
            unsafe {
                libc::kill(-pid, libc::SIGKILL);
            }
        }
    }
}

impl Drop for ProcessGroup {
    /// 【插件进程】【取消回收】Future 取消或执行失败时立即终止仍归属当前调用的进程组。
    /// @returns 无
    fn drop(&mut self) {
        self.terminate();
    }
}

/// 【插件进程】【退出观察】WNOWAIT 保留组长 PID，避免等待输出时 PID 重用造成误杀。
/// @param pid 当前拥有的组长 PID
/// @returns 进程是否已经退出，但不执行回收
fn exited_without_reaping(pid: i32) -> Result<bool> {
    // 【插件进程】【系统调用】1. siginfo 是系统填写的零初始化结构，WNOHANG 不阻塞执行线程
    let mut information: libc::siginfo_t = unsafe { std::mem::zeroed() };
    let result = unsafe {
        libc::waitid(
            libc::P_PID,
            pid as libc::id_t,
            &mut information,
            libc::WEXITED | libc::WNOWAIT | libc::WNOHANG,
        )
    };
    if result != 0 {
        let error = std::io::Error::last_os_error();
        if error.kind() == std::io::ErrorKind::Interrupted {
            return Ok(false);
        }
        return Err(error).context("observe plugin process exit");
    }
    Ok(unsafe { information.si_pid() } != 0)
}
