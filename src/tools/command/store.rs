use crate::runtime_recovery::OwnerKind;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BackgroundCommandTask {
    pub(crate) id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) runtime_process_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) runtime_owner_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) runtime_owner_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) runtime_process_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) goal_id: Option<String>,
    pub(crate) label: String,
    pub(crate) command: String,
    pub(crate) cwd: String,
    pub(crate) pid: u32,
    pub(crate) pgid: Option<i32>,
    pub(crate) status: String,
    pub(crate) stdout_log: String,
    pub(crate) stderr_log: String,
    pub(crate) started_at: u64,
    pub(crate) updated_at: u64,
    pub(crate) timeout_seconds: u64,
    /// 终态完成通知是否已经交给所属会话 Agent
    #[serde(default)]
    pub(crate) completion_notified: bool,
}

impl BackgroundCommandTask {
    /// 判断任务是否归属指定的交互式会话。
    ///
    /// 后台任务存在全局的 tasks.json 里（进程是机器级资源，`sai ps` 需要总览），
    /// 因此凡是要落到某个会话名下的操作——写 runtime_processes、面板列表、完成
    /// 回执——都必须先过这道判定，否则别的会话的任务会串进来。
    ///
    /// 参数:
    /// - `session_id`: 目标会话标识
    ///
    /// 返回:
    /// - 属于该会话时返回 true
    pub(crate) fn owned_by_session(&self, session_id: &str) -> bool {
        self.runtime_owner_kind.as_deref() == Some(OwnerKind::Session.as_str())
            && self.runtime_owner_id.as_deref() == Some(session_id)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct BackgroundCommandStore {
    root: PathBuf,
}

impl BackgroundCommandStore {
    /// 创建后台命令状态存储。
    ///
    /// 参数:
    /// - `state_dir`: Sai 状态目录
    ///
    /// 返回:
    /// - 后台命令状态存储
    pub(crate) fn new(state_dir: PathBuf) -> Self {
        Self {
            root: state_dir.join("background-commands"),
        }
    }

    /// 初始化状态目录。
    ///
    /// 返回:
    /// - 初始化是否成功
    pub(crate) fn init(&self) -> Result<()> {
        std::fs::create_dir_all(self.logs_dir())?;
        Ok(())
    }

    /// 加载任务列表。
    ///
    /// 文件尾部偶尔会残留上一次写入的碎片：写入被打断、或两个进程先后写
    /// 同一个文件而旧内容更长时都会留下它。严格解析会报成 "trailing
    /// characters" 并让调用方失败——一个后台任务列表读不出来不该拖垮整条
    /// 命令，所以退回取第一个完整的 JSON 数组把数据救回来；实在读不出来
    /// 就按空列表处理，而不是把错误抛给上层。
    ///
    /// 返回:
    /// - 后台任务列表
    pub(crate) fn load(&self) -> Result<Vec<BackgroundCommandTask>> {
        let file = self.state_file();
        if !file.exists() {
            return Ok(Vec::new());
        }
        let raw = std::fs::read_to_string(file)?;
        if let Ok(tasks) = serde_json::from_str::<Vec<BackgroundCommandTask>>(&raw) {
            return Ok(tasks);
        }
        let mut stream =
            serde_json::Deserializer::from_str(&raw).into_iter::<Vec<BackgroundCommandTask>>();
        match stream.next() {
            Some(Ok(tasks)) => Ok(tasks),
            _ => Ok(Vec::new()),
        }
    }

    /// 保存任务列表。
    ///
    /// 参数:
    /// - `tasks`: 后台任务列表
    ///
    /// 返回:
    /// - 保存是否成功
    pub(crate) fn save(&self, tasks: &[BackgroundCommandTask]) -> Result<()> {
        self.init()?;
        // 原子替换：直接写目标文件在写入被打断时会留下半截内容，下一次
        // 读取就会失败。先写临时文件再 rename，读到的永远是完整内容。
        let payload = format!("{}\n", serde_json::to_string_pretty(tasks)?);
        let temp = tempfile::NamedTempFile::new_in(&self.root)?;
        std::fs::write(temp.path(), payload)?;
        temp.persist(self.state_file())
            .map_err(|error| anyhow::anyhow!("failed to persist background tasks: {}", error.error))?;
        Ok(())
    }

    /// 追加或替换任务。
    ///
    /// 参数:
    /// - `task`: 后台任务
    ///
    /// 返回:
    /// - 保存是否成功
    pub(crate) fn upsert(&self, task: BackgroundCommandTask) -> Result<()> {
        let mut tasks = self.load()?;
        if let Some(existing) = tasks.iter_mut().find(|item| item.id == task.id) {
            *existing = task;
        } else {
            tasks.push(task);
        }
        self.save(&tasks)
    }

    /// 返回日志目录。
    ///
    /// 返回:
    /// - 日志目录路径
    pub(crate) fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    /// 返回状态文件路径。
    ///
    /// 返回:
    /// - 状态文件路径
    fn state_file(&self) -> PathBuf {
        self.root.join("tasks.json")
    }
}

/// 返回当前 Unix 时间戳。
///
/// 返回:
/// - 秒级 Unix 时间戳
pub(crate) fn unix_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 文件尾部残留碎片时仍能读出前面的完整任务列表。
    ///
    /// 这是线上真实发生过的一次故障：tasks.json 尾部多了 47 字节残留，
    /// 严格解析报 "trailing characters" 并让整条命令失败。
    #[test]
    fn load_salvages_tasks_despite_a_trailing_fragment() {
        let dir = tempfile::tempdir().unwrap();
        let store = BackgroundCommandStore::new(dir.path().to_path_buf());

        store.save(&[]).unwrap();
        let mut raw = std::fs::read_to_string(store.state_file()).unwrap();
        raw.push_str("\n\"completion_notified\": false\n}\n]");
        std::fs::write(store.state_file(), &raw).unwrap();

        assert!(store.load().unwrap().is_empty(), "a trailing fragment must not fail the load");
    }

    /// 保存走原子替换，读到的永远是完整内容。
    #[test]
    fn save_replaces_the_file_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let store = BackgroundCommandStore::new(dir.path().to_path_buf());

        store.save(&[]).unwrap();
        let path = store.state_file();

        assert!(path.is_file());
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(serde_json::from_str::<Vec<BackgroundCommandTask>>(&raw).is_ok());
    }
}
