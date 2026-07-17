use super::session::TerminalSession;
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

/// 管理当前活动工作区的 PTY 会话。
#[derive(Clone)]
pub(crate) struct TerminalManager {
    sessions: Arc<Mutex<HashMap<String, Arc<TerminalSession>>>>,
}

impl TerminalManager {
    /// 创建空终端管理器。
    pub(crate) fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 创建 PTY 会话。
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
        let session = Arc::new(TerminalSession::spawn(
            id.clone(),
            cwd,
            configured_shell,
            cols.max(1),
            rows.max(1),
        )?);
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
    pub(crate) fn get(&self, id: &str) -> Result<Arc<TerminalSession>> {
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
    pub(crate) fn remove(&self, id: &str) -> Result<bool> {
        let session = self.lock_sessions()?.remove(id);
        if let Some(session) = session {
            session.kill()?;
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
    ) -> Result<std::sync::MutexGuard<'_, HashMap<String, Arc<TerminalSession>>>> {
        self.sessions
            .lock()
            .map_err(|_| anyhow::anyhow!("terminal manager lock is poisoned"))
    }
}
