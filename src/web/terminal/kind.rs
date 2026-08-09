use super::manager::TerminalInfo;
use super::session::TerminalSession;
use super::ssh_session::SshTerminalSession;
use anyhow::Result;
use tokio::sync::broadcast;

/// 终端会话的两种承载方式。
///
/// 本地会话跑在 PTY 上，读写是同步阻塞调用；SSH 会话跑在 russh 通道上，
/// 读写是异步的。二者对外暴露同一组操作，socket 层无需区分。
pub(crate) enum TerminalKind {
    Local(TerminalSession),
    Ssh(SshTerminalSession),
}

impl TerminalKind {
    /// 返回终端摘要。
    pub(crate) fn info(&self) -> TerminalInfo {
        match self {
            Self::Local(session) => session.info(),
            Self::Ssh(session) => session.info(),
        }
    }

    /// 更新终端标签标题。
    ///
    /// 参数:
    /// - `title`: 新标题
    ///
    /// 返回:
    /// - 更新后的终端摘要
    pub(crate) fn rename(&self, title: &str) -> Result<TerminalInfo> {
        match self {
            Self::Local(session) => session.rename(title),
            Self::Ssh(session) => session.rename(title),
        }
    }

    /// 订阅终端输出。
    pub(crate) fn subscribe(&self) -> broadcast::Receiver<Vec<u8>> {
        match self {
            Self::Local(session) => session.subscribe(),
            Self::Ssh(session) => session.subscribe(),
        }
    }

    /// 返回连接前已经产生的终端输出。
    pub(crate) fn replay(&self) -> Vec<u8> {
        match self {
            Self::Local(session) => session.replay(),
            Self::Ssh(session) => session.replay(),
        }
    }

    /// 写入终端输入。
    ///
    /// 参数:
    /// - `bytes`: 原始输入字节
    ///
    /// 返回:
    /// - 无
    pub(crate) async fn write(&self, bytes: &[u8]) -> Result<()> {
        match self {
            Self::Local(session) => session.write(bytes),
            Self::Ssh(session) => session.write(bytes).await,
        }
    }

    /// 调整终端尺寸。
    ///
    /// 参数:
    /// - `cols`: 新列数
    /// - `rows`: 新行数
    ///
    /// 返回:
    /// - 无
    pub(crate) async fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        match self {
            Self::Local(session) => session.resize(cols, rows),
            Self::Ssh(session) => session.resize(cols, rows).await,
        }
    }

    /// 结束终端会话。
    pub(crate) async fn kill(&self) -> Result<()> {
        match self {
            Self::Local(session) => session.kill(),
            Self::Ssh(session) => session.kill().await,
        }
    }
}
