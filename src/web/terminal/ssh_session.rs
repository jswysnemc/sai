use super::manager::TerminalInfo;
use crate::config::SshHostConfig;
use crate::web::ssh::{connect_ssh_session, HostKey, KnownHostStatus, SshConnectOutcome};
use anyhow::Result;
use russh::client;
use russh::ChannelMsg;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, RwLock};
use tokio::sync::broadcast;

const TERMINAL_BROADCAST_CAPACITY: usize = 256;
const TERMINAL_REPLAY_BYTES: usize = 1024 * 1024;

/// 建立 SSH 终端会话的结果。
pub(crate) enum SshCreateOutcome {
    /// 会话已建立
    Created(TerminalInfo),
    /// 主机密钥待用户确认，未建立会话
    HostKeyPending {
        key: Box<HostKey>,
        status: KnownHostStatus,
    },
}

/// 运行在 SSH 通道上的远程终端会话。
pub(crate) struct SshTerminalSession {
    id: String,
    title: Mutex<String>,
    size: Arc<RwLock<(u16, u16)>>,
    /// 通道写半部，输入与改窗经由它；读半部交由后台任务独占，避免读写互相阻塞
    writer: tokio::sync::Mutex<russh::ChannelWriteHalf<client::Msg>>,
    output: broadcast::Sender<Vec<u8>>,
    replay: Arc<Mutex<VecDeque<u8>>>,
    /// 持有连接句柄，drop 后连接随之关闭
    _handle: client::Handle<crate::web::ssh::SshClientHandler>,
}

impl SshTerminalSession {
    /// 连接远端主机并启动交互式 Shell。
    ///
    /// 参数:
    /// - `id`: 终端 ID
    /// - `host`: 主机配置
    /// - `passphrase`: 私钥口令，无口令时传 None
    /// - `cols`: 初始列数
    /// - `rows`: 初始行数
    ///
    /// 返回:
    /// - SSH 终端会话；主机密钥待确认时返回 None 及对应密钥
    pub(super) async fn connect(
        id: String,
        host: &SshHostConfig,
        passphrase: Option<&str>,
        cols: u16,
        rows: u16,
    ) -> Result<std::result::Result<Self, (Box<HostKey>, KnownHostStatus)>> {
        // 1. 完成握手与认证，主机密钥未确认时把密钥带回上层
        let handle = match connect_ssh_session(host, passphrase).await? {
            SshConnectOutcome::Connected(handle) => handle,
            SshConnectOutcome::HostKeyPending { key, status } => {
                return Ok(Err((key, status)))
            }
        };

        // 2. 申请带 PTY 的会话通道并启动登录 Shell
        let channel = handle.channel_open_session().await?;
        channel
            .request_pty(false, "xterm-256color", cols as u32, rows as u32, 0, 0, &[])
            .await?;
        channel.request_shell(false).await?;

        // 3. 指定了远端目录时进入该目录，失败不影响会话可用
        let remote_directory = host.remote_directory.trim();
        if !remote_directory.is_empty() {
            let command = format!("cd {}\n", shell_quote(remote_directory));
            channel.data(command.as_bytes()).await?;
        }

        let (output, _) = broadcast::channel(TERMINAL_BROADCAST_CAPACITY);
        let replay = Arc::new(Mutex::new(VecDeque::new()));

        // 4. 拆分读写两半，读半部独占给后台任务，写半部留给输入与改窗
        let (reader, writer) = channel.split();
        spawn_reader(reader, output.clone(), replay.clone());

        Ok(Ok(Self {
            title: Mutex::new(host.label.clone()),
            id,
            size: Arc::new(RwLock::new((cols, rows))),
            writer: tokio::sync::Mutex::new(writer),
            output,
            replay,
            _handle: handle,
        }))
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
                .unwrap_or_else(|_| "ssh".to_string()),
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
    ///
    /// 返回:
    /// - 无
    pub(super) async fn write(&self, bytes: &[u8]) -> Result<()> {
        self.writer.lock().await.data(bytes).await?;
        Ok(())
    }

    /// 调整远端终端尺寸。
    ///
    /// 参数:
    /// - `cols`: 新列数
    /// - `rows`: 新行数
    ///
    /// 返回:
    /// - 无
    pub(super) async fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        let cols = cols.max(1);
        let rows = rows.max(1);
        self.writer
            .lock()
            .await
            .window_change(cols as u32, rows as u32, 0, 0)
            .await?;
        if let Ok(mut size) = self.size.write() {
            *size = (cols, rows);
        }
        Ok(())
    }

    /// 关闭远端通道。
    pub(super) async fn kill(&self) -> Result<()> {
        self.writer.lock().await.eof().await?;
        Ok(())
    }
}

/// 持续读取远端输出并广播给浏览器。
///
/// 参数:
/// - `reader`: 通道读半部
/// - `output`: 输出广播发送端
/// - `replay`: 回放缓冲
fn spawn_reader(
    mut reader: russh::ChannelReadHalf,
    output: broadcast::Sender<Vec<u8>>,
    replay: Arc<Mutex<VecDeque<u8>>>,
) {
    tokio::spawn(async move {
        while let Some(message) = reader.wait().await {
            match message {
                // stdout 与 stderr 都直接送往终端，远端已按 PTY 合流
                ChannelMsg::Data { data } | ChannelMsg::ExtendedData { data, .. } => {
                    let chunk = data.to_vec();
                    if let Ok(mut replay) = replay.lock() {
                        replay.extend(chunk.iter().copied());
                        while replay.len() > TERMINAL_REPLAY_BYTES {
                            replay.pop_front();
                        }
                    }
                    let _ = output.send(chunk);
                }
                ChannelMsg::Eof | ChannelMsg::Close => break,
                _ => {}
            }
        }
    });
}

/// 用单引号包裹路径，避免远端 Shell 对空格与元字符做二次解释。
///
/// 参数:
/// - `value`: 原始路径
///
/// 返回:
/// - 可安全嵌入 Shell 命令的字面量
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::shell_quote;

    #[test]
    fn quotes_a_plain_path() {
        assert_eq!(shell_quote("/srv/app"), "'/srv/app'");
    }

    #[test]
    fn escapes_embedded_single_quotes() {
        assert_eq!(shell_quote("/srv/it's"), r"'/srv/it'\''s'");
    }

    #[test]
    fn keeps_shell_metacharacters_literal() {
        assert_eq!(shell_quote("/srv/a b;rm -rf /"), "'/srv/a b;rm -rf /'");
    }
}
