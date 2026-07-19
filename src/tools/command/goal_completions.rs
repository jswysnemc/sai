use super::background_tasks::{read_log_tail, refresh_task_statuses};
use super::store::{BackgroundCommandStore, BackgroundCommandTask};
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::runtime_recovery::OwnerKind;
use anyhow::Result;

/// 后台命令完成通知摘要。
#[derive(Debug, Clone)]
pub(crate) struct BackgroundCompletionNotice {
    pub(crate) task_id: String,
    pub(crate) label: String,
    pub(crate) status: String,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

/// 查询指定会话 Goal 的后台命令完成事件。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 应用配置
/// - `session_id`: 会话标识
/// - `goal_id`: Goal 标识
///
/// 返回:
/// - 完成通知列表和仍在运行的任务数量
pub(crate) async fn poll_background_completions(
    paths: &SaiPaths,
    config: &AppConfig,
    session_id: &str,
    goal_id: &str,
) -> Result<(Vec<BackgroundCompletionNotice>, usize)> {
    poll_background_completions_matching(
        paths,
        config,
        |task| owned_by_goal(task, session_id, goal_id),
    )
    .await
}

/// 查询指定会话中未绑定 Goal 的后台命令完成事件。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 应用配置
/// - `session_id`: 会话标识
///
/// 返回:
/// - 完成通知列表和仍在运行的任务数量
pub(crate) async fn poll_session_background_completions(
    paths: &SaiPaths,
    config: &AppConfig,
    session_id: &str,
) -> Result<(Vec<BackgroundCompletionNotice>, usize)> {
    poll_background_completions_matching(paths, config, |task| {
        owned_by_session(task, session_id) && task.goal_id.is_none()
    })
    .await
}

/// 按任务归属条件查询后台命令完成事件。
async fn poll_background_completions_matching(
    paths: &SaiPaths,
    config: &AppConfig,
    matches: impl Fn(&BackgroundCommandTask) -> bool,
) -> Result<(Vec<BackgroundCompletionNotice>, usize)> {
    let store = BackgroundCommandStore::new(paths.state_dir.clone());
    let mut tasks = store.load()?;
    let mut changed = false;
    for task in tasks.iter_mut().filter(|task| matches(task)) {
        changed |= refresh_task_statuses(std::slice::from_mut(task), config).await;
    }
    if changed {
        store.save(&tasks)?;
    }
    let mut notices = Vec::new();
    let mut running = 0;
    for task in tasks.iter().filter(|task| matches(task)) {
        if task.status == "running" {
            running += 1;
            continue;
        }
        if task.completion_notified {
            continue;
        }
        let max_bytes = config.tools.background_command_log_max_bytes;
        let stdout = read_log_tail(&task.stdout_log, 200, max_bytes)
            .map(|tail| bounded_completion_output(&tail.text))
            .unwrap_or_default();
        let stderr = read_log_tail(&task.stderr_log, 200, max_bytes)
            .map(|tail| bounded_completion_output(&tail.text))
            .unwrap_or_default();
        notices.push(BackgroundCompletionNotice {
            task_id: task.id.clone(),
            label: task.label.clone(),
            status: task.status.clone(),
            stdout,
            stderr,
        });
    }
    Ok((notices, running))
}

/// 确认后台命令完成通知已经交给 Agent。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `session_id`: 会话标识
/// - `task_ids`: 已消费任务 ID
///
/// 返回:
/// - 保存是否成功
pub(crate) fn acknowledge_background_completions(
    paths: &SaiPaths,
    session_id: &str,
    task_ids: &[String],
) -> Result<()> {
    if task_ids.is_empty() {
        return Ok(());
    }
    let store = BackgroundCommandStore::new(paths.state_dir.clone());
    let mut tasks = store.load()?;
    for task in tasks.iter_mut() {
        if task_ids.iter().any(|id| id == &task.id)
            && owned_by_session(task, session_id)
            && task.status != "running"
        {
            task.completion_notified = true;
        }
    }
    store.save(&tasks)
}

/// 判断任务是否绑定到交互式会话。
fn owned_by_session(task: &BackgroundCommandTask, session_id: &str) -> bool {
    task.runtime_owner_kind.as_deref() == Some(OwnerKind::Session.as_str())
        && task.runtime_owner_id.as_deref() == Some(session_id)
}

/// 判断后台任务是否属于指定会话 Goal。
fn owned_by_goal(task: &BackgroundCommandTask, session_id: &str, goal_id: &str) -> bool {
    owned_by_session(task, session_id) && task.goal_id.as_deref() == Some(goal_id)
}

/// 限制后台命令输出进入模型上下文的长度。
fn bounded_completion_output(text: &str) -> String {
    const LIMIT: usize = 4_000;
    if text.chars().count() <= LIMIT {
        return text.to_string();
    }
    let mut result = text.chars().take(LIMIT).collect::<String>();
    result.push_str("\n[后台命令输出已截断]");
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_paths(state_dir: PathBuf) -> SaiPaths {
        SaiPaths {
            config_dir: PathBuf::new(),
            config_file: PathBuf::new(),
            secrets_file: PathBuf::new(),
            skills_dir: PathBuf::new(),
            data_dir: PathBuf::new(),
            cache_dir: PathBuf::new(),
            state_dir,
            pictures_dir: PathBuf::new(),
            fish_hook_file: PathBuf::new(),
            bash_hook_file: PathBuf::new(),
            zsh_hook_file: PathBuf::new(),
            powershell_hook_file: PathBuf::new(),
        }
    }

    /// 验证 Goal 后台命令完成通知只投递一次。
    #[tokio::test]
    async fn goal_background_completion_is_acknowledged_once() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        let store = BackgroundCommandStore::new(paths.state_dir.clone());
        store.init().unwrap();
        let stdout_log = store.logs_dir().join("goal-task.out.log");
        let stderr_log = store.logs_dir().join("goal-task.err.log");
        std::fs::write(&stdout_log, "tests passed\n").unwrap();
        std::fs::write(&stderr_log, "").unwrap();
        store
            .save(&[BackgroundCommandTask {
                id: "goal-task".to_string(),
                runtime_process_id: None,
                runtime_owner_kind: Some("session".to_string()),
                runtime_owner_id: Some("session-1".to_string()),
                runtime_process_kind: Some("background_command".to_string()),
                goal_id: Some("goal-1".to_string()),
                label: "test suite".to_string(),
                command: "cargo test".to_string(),
                cwd: ".".to_string(),
                pid: 123,
                pgid: None,
                status: "exited".to_string(),
                stdout_log: stdout_log.display().to_string(),
                stderr_log: stderr_log.display().to_string(),
                started_at: 1,
                updated_at: 2,
                timeout_seconds: 30,
                completion_notified: false,
            }])
            .unwrap();
        let mut tasks = store.load().unwrap();
        let mut unrelated = tasks[0].clone();
        unrelated.id = "other-goal-task".to_string();
        unrelated.goal_id = Some("goal-2".to_string());
        unrelated.status = "running".to_string();
        unrelated.pid = u32::MAX;
        tasks.push(unrelated);
        store.save(&tasks).unwrap();

        let (first, running) =
            poll_background_completions(&paths, &AppConfig::default(), "session-1", "goal-1")
                .await
                .unwrap();
        assert_eq!(running, 0);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].stdout, "tests passed");
        assert_eq!(
            store
                .load()
                .unwrap()
                .iter()
                .find(|task| task.id == "other-goal-task")
                .unwrap()
                .status,
            "running"
        );

        let mut all_tasks = store.load().unwrap();
        let mut session_task = all_tasks
            .iter()
            .find(|task| task.id == "goal-task")
            .cloned()
            .unwrap();
        session_task.id = "session-task".to_string();
        session_task.goal_id = None;
        session_task.completion_notified = false;
        all_tasks.push(session_task);
        store.save(&all_tasks).unwrap();
        let (session_notices, session_running) = poll_session_background_completions(
            &paths,
            &AppConfig::default(),
            "session-1",
        )
        .await
        .unwrap();
        assert_eq!(session_running, 0);
        assert_eq!(session_notices.len(), 1);
        assert_eq!(session_notices[0].task_id, "session-task");

        acknowledge_background_completions(&paths, "session-1", &["goal-task".to_string()])
            .unwrap();
        let (second, _) =
            poll_background_completions(&paths, &AppConfig::default(), "session-1", "goal-1")
                .await
                .unwrap();
        assert!(second.is_empty());
    }
}
