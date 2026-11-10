use super::output::{read_output, OutputBuffer};
use crate::sandbox::SandboxPolicy;
use anyhow::{Context, Result};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// 【交互命令】【PTY 生命周期】输入、输出和整棵进程树共享同一会话，退出和取消均回收后代。
pub(super) struct InteractiveSession {
    pub(super) owner: String,
    pub(super) pid: u32,
    pub(super) command: String,
    pub(super) cwd: std::path::PathBuf,
    pub(super) started_at: u64,
    _master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    child: Mutex<Box<dyn Child + Send + Sync>>,
    tree: Mutex<crate::tools::command::process_lifecycle::CommandTreeGuard>,
    output: Arc<Mutex<OutputBuffer>>,
    exit_code: Mutex<Option<u32>>,
    pub(super) operation: tokio::sync::Mutex<()>,
}

impl InteractiveSession {
    /// 【交互命令】【PTY 启动】复用实际 Shell 与沙箱包装，不通过 PTY 绕过文件或网络策略。
    /// 参数：归属、命令、Shell、目录及策略；返回可交互的会话。
    pub(super) fn spawn(
        owner: String,
        command: &str,
        shell: &str,
        cwd: &Path,
        sandbox: Option<&SandboxPolicy>,
    ) -> Result<Self> {
        let mut failures = Vec::new();
        for (name, process) in crate::tools::command::build_shell_commands(command, shell, sandbox)?
        {
            let process = process.as_std();
            let pair = native_pty_system().openpty(PtySize {
                rows: 24,
                cols: 100,
                pixel_width: 0,
                pixel_height: 0,
            })?;
            let mut builder = CommandBuilder::new(process.get_program());
            builder.args(process.get_args());
            builder.cwd(process.get_current_dir().unwrap_or(cwd));
            builder.env_clear();
            for (key, value) in process.get_envs() {
                if let Some(value) = value {
                    builder.env(key, value);
                }
            }
            builder.env("TERM", "xterm-256color");
            let reader = pair.master.try_clone_reader()?;
            let mut writer = pair.master.take_writer()?;
            if cfg!(windows) {
                // 1. 【交互命令】【ConPTY 初始化】回应光标查询，避免启动时等待真实终端回应
                writer.write_all(b"\x1b[1;1R")?;
                writer.flush()?;
            }
            let mut child = match pair.slave.spawn_command(builder) {
                Ok(child) => child,
                Err(error) => {
                    failures.push(format!("{name}: {error:#}"));
                    continue;
                }
            };
            drop(pair.slave);
            let pid = child
                .process_id()
                .context("interactive command has no process id")?;
            let tree = match crate::tools::command::process_lifecycle::CommandTreeGuard::new(pid) {
                Ok(tree) => tree,
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(error);
                }
            };
            let output = Arc::new(Mutex::new(OutputBuffer::default()));
            let read_buffer = output.clone();
            if let Err(error) = std::thread::Builder::new()
                .name(format!("sai-command-{pid}"))
                .spawn(move || read_output(reader, read_buffer))
            {
                drop(tree);
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.into());
            }
            return Ok(Self {
                owner,
                pid,
                command: command.to_string(),
                cwd: cwd.to_path_buf(),
                started_at: crate::tools::command::unix_seconds(),
                _master: Mutex::new(pair.master),
                writer: Mutex::new(writer),
                child: Mutex::new(child),
                tree: Mutex::new(tree),
                output,
                exit_code: Mutex::new(None),
                operation: tokio::sync::Mutex::new(()),
            });
        }
        anyhow::bail!(
            "failed to start an interactive shell: {}",
            failures.join("; ")
        )
    }

    /// 【交互命令】【输入写入】向原 PTY 写入文本；参数为输入字节，返回写入结果。
    pub(super) fn write(&self, text: &str) -> Result<()> {
        let mut writer = self
            .writer
            .lock()
            .map_err(|_| anyhow::anyhow!("interactive writer lock poisoned"))?;
        writer.write_all(text.as_bytes())?;
        writer.flush()?;
        Ok(())
    }

    /// 【交互命令】【退出检查】发现 Shell 退出时清理剩余后代，避免 PTY 持续等待 EOF。
    /// 参数：无；返回退出码和读取端是否已结束。
    pub(super) fn status(&self) -> Result<(Option<u32>, bool)> {
        let mut exit_code = self
            .exit_code
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if exit_code.is_none() {
            if let Some(status) = self
                .child
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .try_wait()?
            {
                *exit_code = Some(status.exit_code());
                self.terminate();
            }
        }
        Ok((
            *exit_code,
            self.output
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .eof,
        ))
    }

    /// 【交互命令】【输出读取】取走尚未交付的尾部输出；无参数，返回文本和省略字节数。
    pub(super) fn take_output(&self) -> (String, usize) {
        self.output
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
    }

    /// 【交互命令】【只读快照】查看面板输出不消费模型的未读结果；无参数，返回文本与截断标记。
    pub(super) fn peek_output(&self) -> (String, bool) {
        self.output
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .peek()
    }

    /// 【交互命令】【取消清理】终止整棵进程树；无参数、无返回值。
    pub(super) fn terminate(&self) {
        self.tree
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .terminate();
    }
}

impl Drop for InteractiveSession {
    /// 【交互命令】【会话回收】释放所有句柄前终止并回收子进程；无参数、无返回值。
    fn drop(&mut self) {
        self.terminate();
        if let Ok(child) = self.child.get_mut() {
            let _ = child.wait();
        }
    }
}
