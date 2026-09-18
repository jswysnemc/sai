use super::ownership::{process_exists, SessionOwner};
use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 【会话在线】【实例记录】仅用于网格探测，不授予独占权，不包含转交端点。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct SessionPresence {
    pub session_id: String,
    pub owner: String,
    pub pid: u32,
    pub started_at: String,
}

/// 【会话在线】【生命周期】每个界面独立登记，释放时只移除自己的文件。
pub(crate) struct SessionPresenceGuard {
    path: PathBuf,
}

impl SessionPresenceGuard {
    /// 【会话在线】【登记】允许多个实例打开同一会话，不读取旧持有者登记。
    /// 参数: state_dir 为会话目录，session_id 为标识，owner 为界面类型
    /// 返回: 当前实例的在线记录守卫；失败不影响本地输入执行
    pub(crate) fn register(
        state_dir: &Path,
        session_id: &str,
        owner: SessionOwner,
    ) -> Result<Self> {
        let directory = state_dir.join("session-presence");
        std::fs::create_dir_all(&directory)?;
        let record = SessionPresence {
            session_id: session_id.into(),
            owner: owner.as_str().into(),
            pid: std::process::id(),
            started_at: Utc::now().to_rfc3339(),
        };
        let path = directory.join(format!(
            "{}-{}.json",
            record.pid,
            uuid::Uuid::new_v4().simple()
        ));
        let temporary = tempfile::NamedTempFile::new_in(&directory)?;
        serde_json::to_writer(temporary.as_file(), &record)?;
        temporary.persist(&path)?;
        Ok(Self { path })
    }

    /// 【会话在线】【目录匹配】判断是否仍在记录当前会话，切换后由调用方重新登记。
    /// 参数: state_dir 为当前会话目录；返回是否匹配
    pub(crate) fn matches(&self, state_dir: &Path) -> bool {
        self.path.parent().and_then(Path::parent) == Some(state_dir)
    }
}

impl Drop for SessionPresenceGuard {
    /// 【会话在线】【退出清理】释放当前记录，不修改其他实例；无参数，无返回值。
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// 【会话在线】【只读探测】列出仍存活的实例，忽略退出进程和不可解析记录。
/// 参数: state_dir 为会话目录；返回按登记时间排序的在线实例
pub(crate) fn session_instances(state_dir: &Path) -> Vec<SessionPresence> {
    let Ok(entries) = std::fs::read_dir(state_dir.join("session-presence")) else {
        return Vec::new();
    };
    let mut records = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            if entry
                .path()
                .extension()
                .is_none_or(|extension| extension != "json")
            {
                return None;
            }
            let metadata = entry.metadata().ok()?;
            if !metadata.is_file() || metadata.len() > 8192 {
                return None;
            }
            let record: SessionPresence =
                serde_json::from_slice(&std::fs::read(entry.path()).ok()?).ok()?;
            process_exists(record.pid).then_some(record)
        })
        .collect::<Vec<_>>();
    records.sort_by(|left, right| left.started_at.cmp(&right.started_at));
    records
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【会话在线】【多实例】多个界面互不排斥，释放一个实例不影响另一个。
    #[test]
    fn multiple_instances_are_nonexclusive_and_cleanup_independently() {
        let root = tempfile::tempdir().unwrap();
        let first =
            SessionPresenceGuard::register(root.path(), "session", SessionOwner::Repl).unwrap();
        let second =
            SessionPresenceGuard::register(root.path(), "session", SessionOwner::Web).unwrap();
        assert_eq!(session_instances(root.path()).len(), 2);
        let run = super::super::ActiveRunGuard::acquire_with_state_dir(
            "session",
            SessionOwner::Repl,
            root.path(),
        )
        .unwrap();
        assert!(super::super::ActiveRunGuard::acquire_with_state_dir(
            "session",
            SessionOwner::Web,
            root.path()
        )
        .is_err());
        drop(run);
        drop(first);
        assert_eq!(session_instances(root.path()).len(), 1);
        assert!(second.matches(root.path()));
        drop(second);
        assert!(session_instances(root.path()).is_empty());
    }

    /// 【会话在线】【旧版兼容】旧持有者文件和退出进程不影响新实例打开会话。
    #[test]
    fn legacy_holder_and_dead_presence_do_not_control_local_execution() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("session-holder.json"),
            format!(r#"{{"pid":{},"owner":"repl"}}"#, std::process::id()),
        )
        .unwrap();
        let guard =
            SessionPresenceGuard::register(root.path(), "session", SessionOwner::Repl).unwrap();
        std::fs::write(
            root.path().join("session-presence/dead.json"),
            serde_json::to_vec(&SessionPresence {
                session_id: "session".into(),
                owner: "repl".into(),
                pid: u32::MAX,
                started_at: "old".into(),
            })
            .unwrap(),
        )
        .unwrap();
        assert_eq!(session_instances(root.path()).len(), 1);
        drop(guard);
        assert!(session_instances(root.path()).is_empty());
        assert!(root.path().join("session-holder.json").exists());
    }
}
