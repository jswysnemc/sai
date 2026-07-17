use super::manager::TerminalInfo;
use anyhow::{Context, Result};
use portable_pty::{native_pty_system, Child, MasterPty, PtySize};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};
use tokio::sync::broadcast;

const TERMINAL_BROADCAST_CAPACITY: usize = 256;
const TERMINAL_REPLAY_BYTES: usize = 1024 * 1024;

/// 单个 PTY 会话及其输入输出通道。
pub(crate) struct TerminalSession {
    id: String,
    title: Mutex<String>,
    size: Arc<RwLock<(u16, u16)>>,
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    child: Mutex<Box<dyn Child + Send>>,
    output: broadcast::Sender<Vec<u8>>,
    replay: Arc<Mutex<VecDeque<u8>>>,
}

impl TerminalSession {
    /// 在指定目录启动用户 Shell。
    ///
    /// 参数:
    /// - `id`: 终端 ID
    /// - `cwd`: 工作目录
    /// - `configured_shell`: 用户配置的 Shell 可执行文件路径或名称
    /// - `cols`: 初始列数
    /// - `rows`: 初始行数
    ///
    /// 返回:
    /// - PTY 会话
    pub(super) fn spawn(
        id: String,
        cwd: &Path,
        configured_shell: &str,
        cols: u16,
        rows: u16,
    ) -> Result<Self> {
        let pair = native_pty_system().openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        let mut command = super::startup::terminal_command(configured_shell);
        let cwd = crate::platform::windows_path::simplified(cwd);
        command.cwd(&cwd);
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        command.env("TERM_PROGRAM", "Sai Web");
        let shell_label = command
            .get_argv()
            .first()
            .map(|program| program.to_string_lossy().into_owned())
            .unwrap_or_else(|| "user login shell".to_string());
        let mut reader = pair.master.try_clone_reader()?;
        let mut writer = pair.master.take_writer()?;
        super::startup::initialize_pty_writer(&mut writer, cfg!(windows))?;
        let child = pair
            .slave
            .spawn_command(command)
            .with_context(|| format!("failed to start terminal shell {shell_label}"))?;
        drop(pair.slave);
        let (output, _) = broadcast::channel(TERMINAL_BROADCAST_CAPACITY);
        let output_thread = output.clone();
        let replay = Arc::new(Mutex::new(VecDeque::new()));
        let replay_thread = replay.clone();
        std::thread::Builder::new()
            .name(format!("sai-terminal-{id}"))
            .spawn(move || read_terminal_output(&mut reader, &output_thread, &replay_thread))?;
        Ok(Self {
            title: Mutex::new(
                cwd.file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("terminal")
                    .to_string(),
            ),
            id,
            size: Arc::new(RwLock::new((cols, rows))),
            master: Mutex::new(pair.master),
            writer: Mutex::new(writer),
            child: Mutex::new(child),
            output,
            replay,
        })
    }

    /// 返回终端摘要。
    pub(super) fn info(&self) -> TerminalInfo {
        let (cols, rows) = self.size.read().map(|size| *size).unwrap_or((80, 24));
        TerminalInfo {
            id: self.id.clone(),
            title: self
                .title
                .lock()
                .map(|title| title.clone())
                .unwrap_or_else(|_| "terminal".to_string()),
            cols,
            rows,
        }
    }

    /// 更新终端标签标题。
    ///
    /// 参数:
    /// - `title`: 新标题
    ///
    /// 返回:
    /// - 更新后的终端摘要
    pub(super) fn rename(&self, title: &str) -> Result<TerminalInfo> {
        let title = title.trim();
        if title.is_empty() {
            anyhow::bail!("terminal title cannot be empty");
        }
        *self
            .title
            .lock()
            .map_err(|_| anyhow::anyhow!("terminal title lock is poisoned"))? = title.to_string();
        Ok(self.info())
    }

    /// 订阅终端输出。
    pub(super) fn subscribe(&self) -> broadcast::Receiver<Vec<u8>> {
        self.output.subscribe()
    }

    /// 返回连接前已经产生的终端输出。
    pub(super) fn replay(&self) -> Vec<u8> {
        self.replay
            .lock()
            .map(|buffer| buffer.iter().copied().collect())
            .unwrap_or_default()
    }

    /// 写入终端输入。
    ///
    /// 参数:
    /// - `bytes`: 原始输入字节
    pub(super) fn write(&self, bytes: &[u8]) -> Result<()> {
        let mut writer = self
            .writer
            .lock()
            .map_err(|_| anyhow::anyhow!("terminal writer lock is poisoned"))?;
        writer.write_all(bytes)?;
        writer.flush()?;
        Ok(())
    }

    /// 调整终端尺寸。
    ///
    /// 参数:
    /// - `cols`: 新列数
    /// - `rows`: 新行数
    pub(super) fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        let cols = cols.max(1);
        let rows = rows.max(1);
        self.master
            .lock()
            .map_err(|_| anyhow::anyhow!("terminal master lock is poisoned"))?
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })?;
        if let Ok(mut size) = self.size.write() {
            *size = (cols, rows);
        }
        Ok(())
    }

    /// 终止终端子进程。
    pub(super) fn kill(&self) -> Result<()> {
        self.child
            .lock()
            .map_err(|_| anyhow::anyhow!("terminal child lock is poisoned"))?
            .kill()?;
        Ok(())
    }
}

/// 持续读取 PTY 输出并广播给浏览器。
fn read_terminal_output(
    reader: &mut Box<dyn Read + Send>,
    output: &broadcast::Sender<Vec<u8>>,
    replay: &Arc<Mutex<VecDeque<u8>>>,
) {
    let mut buffer = [0u8; 8192];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(read) => {
                let chunk = buffer[..read].to_vec();
                if let Ok(mut replay) = replay.lock() {
                    replay.extend(chunk.iter().copied());
                    while replay.len() > TERMINAL_REPLAY_BYTES {
                        replay.pop_front();
                    }
                }
                let _ = output.send(chunk);
            }
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// 验证 Windows ConPTY 能够启动 PowerShell 并返回命令输出。
    #[test]
    fn powershell_session_emits_output() {
        let cwd = std::env::current_dir().expect("应能读取当前目录");
        let session = TerminalSession::spawn(
            "terminal_windows_test".to_string(),
            &cwd,
            "powershell.exe",
            80,
            24,
        )
        .expect("PowerShell 终端应成功启动");

        session
            .write(b"Write-Output __SAI_PTY_READY__\r\n")
            .expect("应能写入 PowerShell 命令");
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut ready = false;
        while Instant::now() < deadline {
            if String::from_utf8_lossy(&session.replay()).contains("__SAI_PTY_READY__") {
                ready = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = session.kill();

        assert!(ready, "PowerShell 未在五秒内返回终端输出");
    }
}
