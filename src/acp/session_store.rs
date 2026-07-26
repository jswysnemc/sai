use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 持久化的 ACP 会话标识。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct StoredAcpSession {
    /// 产生该会话的内核；换内核后旧标识无效
    engine: String,
    /// 外部 agent 分配的会话标识
    session_id: String,
}

/// ACP 会话标识的落盘存储。
///
/// 外部 agent 自己维护对话历史，sai 这边只需记住它给的会话标识，
/// 下次连接时用 `session/load` 接回去——否则每次启动都是一段空白对话，
/// 与 sai 会话列表里看到的历史对不上。
#[derive(Clone)]
pub(crate) struct AcpSessionStore {
    file: PathBuf,
}

impl AcpSessionStore {
    /// 在会话状态目录下创建存储。
    ///
    /// 参数:
    /// - `state_dir`: 当前会话的状态目录
    ///
    /// 返回:
    /// - 存储句柄
    pub(crate) fn new(state_dir: &Path) -> Self {
        Self {
            file: state_dir.join("acp-session.json"),
        }
    }

    /// 读取该内核上次留下的会话标识。
    ///
    /// 参数:
    /// - `engine`: 当前内核标识
    ///
    /// 返回:
    /// - 同一内核留下的会话标识；无记录或内核不符时为 None
    pub(crate) fn load(&self, engine: &str) -> Option<String> {
        let content = std::fs::read_to_string(&self.file).ok()?;
        let stored = serde_json::from_str::<StoredAcpSession>(&content).ok()?;
        // 换内核后旧标识对新 agent 没有意义，直接忽略
        (stored.engine == engine).then_some(stored.session_id)
    }

    /// 记录当前会话标识。
    ///
    /// 参数:
    /// - `engine`: 当前内核标识
    /// - `session_id`: 外部 agent 分配的会话标识
    ///
    /// 返回:
    /// - 写入结果
    pub(crate) fn save(&self, engine: &str, session_id: &str) -> Result<()> {
        if let Some(parent) = self.file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let stored = StoredAcpSession {
            engine: engine.to_string(),
            session_id: session_id.to_string(),
        };
        std::fs::write(&self.file, serde_json::to_string(&stored)?)?;
        Ok(())
    }

    /// 丢弃已失效的会话标识。
    ///
    /// 恢复失败时调用，避免每次启动都拿同一个坏标识去试。
    ///
    /// 返回:
    /// - 无
    pub(crate) fn clear(&self) {
        let _ = std::fs::remove_file(&self.file);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_the_session_id() {
        let dir = tempfile::tempdir().unwrap();
        let store = AcpSessionStore::new(dir.path());

        assert!(store.load("codex").is_none());
        store.save("codex", "sess-1").unwrap();
        assert_eq!(store.load("codex").as_deref(), Some("sess-1"));
    }

    /// 换内核后旧标识对新 agent 无意义，必须忽略而不是拿去 load。
    #[test]
    fn ignores_a_session_from_another_engine() {
        let dir = tempfile::tempdir().unwrap();
        let store = AcpSessionStore::new(dir.path());
        store.save("codex", "sess-1").unwrap();

        assert!(store.load("claude_code").is_none());
    }

    #[test]
    fn clear_drops_the_record() {
        let dir = tempfile::tempdir().unwrap();
        let store = AcpSessionStore::new(dir.path());
        store.save("codex", "sess-1").unwrap();

        store.clear();

        assert!(store.load("codex").is_none());
    }

    /// 文件损坏时按无记录处理，不该让会话起不来。
    #[test]
    fn treats_corrupt_records_as_missing() {
        let dir = tempfile::tempdir().unwrap();
        let store = AcpSessionStore::new(dir.path());
        std::fs::write(dir.path().join("acp-session.json"), "{not json").unwrap();

        assert!(store.load("codex").is_none());
    }
}
