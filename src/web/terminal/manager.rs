use super::kind::TerminalKind;
use super::session::TerminalSession;
use super::ssh_session::{SshCreateOutcome, SshTerminalSession};
use crate::config::SshHostConfig;
use anyhow::Result;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// 浏览器终端摘要。
#[derive(Clone, Debug, Serialize)]
pub(crate) struct TerminalInfo {
    pub id: String,
    pub title: String,
    pub cols: u16,
    pub rows: u16,
}

/// 管理当前活动工作区的终端会话。
#[derive(Clone)]
pub(crate) struct TerminalManager {
    sessions: Arc<Mutex<HashMap<String, Arc<TerminalKind>>>>,
}

impl TerminalManager {
    /// 创建空终端管理器。
    pub(crate) fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 创建本地 PTY 会话。
    ///
    /// 参数:
    /// - `cwd`: 启动目录
    /// - `configured_shell`: 用户配置的 Shell 可执行文件路径或名称
    /// - `cols`: 初始列数
    /// - `rows`: 初始行数
    ///
    /// 返回:
    /// - 终端摘要
    pub(crate) fn create(
        &self,
        cwd: &Path,
        configured_shell: &str,
        cols: u16,
        rows: u16,
    ) -> Result<TerminalInfo> {
        let id = format!("term_{}", uuid::Uuid::new_v4().simple());
        let session = TerminalKind::Local(TerminalSession::spawn(
            id.clone(),
            cwd,
            configured_shell,
            cols.max(1),
            rows.max(1),
        )?);
        self.insert(id, session)
    }

    /// 创建 SSH 远程会话。
    ///
    /// 参数:
    /// - `host`: 主机配置
    /// - `passphrase`: 私钥口令，无口令时传 None
    /// - `cols`: 初始列数
    /// - `rows`: 初始行数
    ///
    /// 返回:
    /// - 终端摘要；主机密钥待确认时返回待确认结果
    pub(crate) async fn create_ssh(
        &self,
        host: &SshHostConfig,
        passphrase: Option<&str>,
        cols: u16,
        rows: u16,
    ) -> Result<SshCreateOutcome> {
        let id = format!("ssh_{}", uuid::Uuid::new_v4().simple());
        match SshTerminalSession::connect(id.clone(), host, passphrase, cols.max(1), rows.max(1))
            .await?
        {
            Ok(session) => Ok(SshCreateOutcome::Created(
                self.insert(id, TerminalKind::Ssh(session))?,
            )),
            Err((key, status)) => Ok(SshCreateOutcome::HostKeyPending { key, status }),
        }
    }

    /// 登记会话并返回摘要。
    ///
    /// 参数:
    /// - `id`: 终端 ID
    /// - `session`: 终端会话
    ///
    /// 返回:
    /// - 终端摘要
    fn insert(&self, id: String, session: TerminalKind) -> Result<TerminalInfo> {
        let session = Arc::new(session);
        let info = session.info();
        self.lock_sessions()?.insert(id, session);
        Ok(info)
    }

    /// 返回全部终端摘要。
    pub(crate) fn list(&self) -> Result<Vec<TerminalInfo>> {
        Ok(self
            .lock_sessions()?
            .values()
            .map(|session| session.info())
            .collect())
    }

    /// 返回指定终端会话。
    pub(crate) fn get(&self, id: &str) -> Result<Arc<TerminalKind>> {
        self.lock_sessions()?
            .get(id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("terminal not found: {id}"))
    }

    /// 重命名指定终端标签。
    ///
    /// 参数:
    /// - `id`: 终端 ID
    /// - `title`: 新标题
    ///
    /// 返回:
    /// - 更新后的终端摘要
    pub(crate) fn rename(&self, id: &str, title: &str) -> Result<TerminalInfo> {
        self.get(id)?.rename(title)
    }

    /// 终止并移除终端。
    ///
    /// 参数:
    /// - `id`: 终端 ID
    ///
    /// 返回:
    /// - 是否完成移除
    pub(crate) async fn remove(&self, id: &str) -> Result<bool> {
        let session = self.lock_sessions()?.remove(id);
        if let Some(session) = session {
            session.kill().await?;
            return Ok(true);
        }
        Ok(false)
    }

    /// 判断是否存在活动终端。
    pub(crate) fn has_sessions(&self) -> Result<bool> {
        Ok(!self.lock_sessions()?.is_empty())
    }

    /// 获取终端表锁。
    fn lock_sessions(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, HashMap<String, Arc<TerminalKind>>>> {
        self.sessions
            .lock()
            .map_err(|_| anyhow::anyhow!("terminal manager lock is poisoned"))
    }
}
