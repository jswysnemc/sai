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
    /// 返回:
    /// - 后台任务列表
    pub(crate) fn load(&self) -> Result<Vec<BackgroundCommandTask>> {
        let file = self.state_file();
        if !file.exists() {
            return Ok(Vec::new());
        }
        let raw = std::fs::read_to_string(file)?;
        Ok(serde_json::from_str(&raw)?)
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
        std::fs::write(
            self.state_file(),
            format!("{}\n", serde_json::to_string_pretty(tasks)?),
        )?;
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
