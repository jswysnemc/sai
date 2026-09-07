use super::store::{unix_seconds, BackgroundCommandStore, BackgroundCommandTask};
use crate::paths::SaiPaths;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const QUIET_SECONDS: u64 = 90;
const LONG_RUNNING_SECONDS: u64 = 600;
const REMINDER_INTERVAL_SECONDS: u64 = 300;

/// 仍在运行、需要检查进展的任务摘要。
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct BackgroundAttentionNotice {
    pub(crate) event_id: String,
    pub(crate) task_id: String,
    pub(crate) label: String,
    pub(crate) command: String,
    pub(crate) running_seconds: u64,
    pub(crate) quiet_seconds: u64,
    pub(crate) quiet: bool,
}

/// 运行中提醒独立于终态回执保存，检查日志不会吞掉后续完成通知。
#[derive(Default, Serialize, Deserialize)]
struct AttentionState {
    #[serde(default)]
    checked_at: BTreeMap<String, u64>,
}

/// 【后台命令】【进展提醒】查询属于指定会话和 Goal 的运行中提醒。
///
/// 参数: `paths` 为状态路径，`session_id` 为所属会话，`goal_id` 为空时仅查询普通会话任务
/// 返回: 需要检查的任务，按开始时间排序；查询本身不确认通知
pub(crate) fn poll_background_attention(
    paths: &SaiPaths,
    session_id: &str,
    goal_id: Option<&str>,
) -> Result<Vec<BackgroundAttentionNotice>> {
    let tasks = BackgroundCommandStore::new(paths.state_dir.clone()).load()?;
    let state = load_state(paths)?;
    let now = unix_seconds();
    let mut notices = tasks
        .iter()
        .filter(|task| task.owned_by_session(session_id) && task.goal_id.as_deref() == goal_id)
        .filter_map(|task| notice_for_task(task, state.checked_at.get(&task.id).copied(), now))
        .collect::<Vec<_>>();
    notices.sort_by(|left, right| {
        right
            .running_seconds
            .cmp(&left.running_seconds)
            .then_with(|| left.task_id.cmp(&right.task_id))
    });
    Ok(notices)
}

/// 【后台命令】【进展提醒】确认主 Agent 已处理提醒，或用户已显式查看日志。
///
/// 参数: `paths` 为状态路径，`session_id` 为所属会话，`task_ids` 为已检查任务
/// 返回: 持久化结果；不会修改任务运行状态或完成回执
pub(crate) fn acknowledge_background_attention(
    paths: &SaiPaths,
    session_id: &str,
    task_ids: &[String],
) -> Result<()> {
    if task_ids.is_empty() {
        return Ok(());
    }
    let tasks = BackgroundCommandStore::new(paths.state_dir.clone()).load()?;
    let mut state = load_state(paths)?;
    state
        .checked_at
        .retain(|id, _| tasks.iter().any(|task| &task.id == id));
    for task in tasks
        .iter()
        .filter(|task| task.owned_by_session(session_id) && task_ids.contains(&task.id))
    {
        state.checked_at.insert(task.id.clone(), unix_seconds());
    }
    let root = paths.state_dir.join("background-commands");
    std::fs::create_dir_all(&root)?;
    let temp = tempfile::NamedTempFile::new_in(&root)?;
    std::fs::write(temp.path(), serde_json::to_vec(&state)?)?;
    temp.persist(root.join("attention.json"))
        .map_err(|error| error.error)?;
    Ok(())
}

/// 读取提醒冷却状态；参数为应用路径，返回状态，不存在时使用空值。
fn load_state(paths: &SaiPaths) -> Result<AttentionState> {
    let file = paths.state_dir.join("background-commands/attention.json");
    if !file.exists() {
        return Ok(AttentionState::default());
    }
    Ok(serde_json::from_slice(&std::fs::read(file)?)?)
}

/// 根据日志更新时间计算提醒；参数为任务、上次检查时间和当前秒数，返回可选提醒。
fn notice_for_task(
    task: &BackgroundCommandTask,
    checked_at: Option<u64>,
    now: u64,
) -> Option<BackgroundAttentionNotice> {
    if task.status != "running"
        || checked_at.is_some_and(|at| now.saturating_sub(at) < REMINDER_INTERVAL_SECONDS)
    {
        return None;
    }
    // 1. 只读取元数据，不在高频轮询中读取日志内容
    let last_output = [&task.stdout_log, &task.stderr_log]
        .into_iter()
        .filter_map(|path| {
            std::fs::metadata(path)
                .ok()?
                .modified()
                .ok()?
                .duration_since(std::time::UNIX_EPOCH)
                .ok()
        })
        .map(|duration| duration.as_secs())
        .max()
        .unwrap_or(task.started_at)
        .max(task.started_at);
    let quiet_seconds = now.saturating_sub(last_output);
    let running_seconds = now.saturating_sub(task.started_at);
    let quiet = quiet_seconds >= QUIET_SECONDS;
    // 2. 即使持续输出，长时间运行也会提示检查；是否终止由主 Agent 和用户决定
    (quiet || running_seconds >= LONG_RUNNING_SECONDS).then(|| BackgroundAttentionNotice {
        event_id: format!(
            "background-attention:{}:{}",
            task.id,
            checked_at.unwrap_or(task.started_at)
        ),
        task_id: task.id.clone(),
        label: task.label.clone(),
        command: task.command.clone(),
        running_seconds,
        quiet_seconds,
        quiet,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造没有日志文件的测试任务；参数为开始时间，返回运行中任务。
    fn task(started_at: u64) -> BackgroundCommandTask {
        serde_json::from_value(serde_json::json!({
            "id":"test", "label":"build", "command":"build", "cwd":".", "pid":1,
            "status":"running", "stdout_log":"", "stderr_log":"", "started_at":started_at,
            "updated_at":started_at, "timeout_seconds":0
        }))
        .unwrap()
    }

    /// 无输出达到阈值后提醒，确认后的冷却期内不重复提醒。
    #[test]
    fn quiet_tasks_observe_threshold_and_cooldown() {
        let task = task(1_000);
        assert!(notice_for_task(&task, None, 1_089).is_none());
        assert!(notice_for_task(&task, None, 1_090).unwrap().quiet);
        assert!(notice_for_task(&task, Some(1_090), 1_389).is_none());
        assert!(notice_for_task(&task, Some(1_090), 1_390).is_some());
    }

    /// 持续输出只在长时间运行后提醒，已结束的任务不产生运行中提醒。
    #[test]
    fn active_output_defers_attention_but_not_forever() {
        let temp = tempfile::NamedTempFile::new().unwrap();
        let now = unix_seconds();
        let mut task = task(now.saturating_sub(LONG_RUNNING_SECONDS - 1));
        task.stdout_log = temp.path().display().to_string();
        assert!(notice_for_task(&task, None, now).is_none());
        task.started_at = now.saturating_sub(LONG_RUNNING_SECONDS);
        assert!(!notice_for_task(&task, None, now).unwrap().quiet);
        task.status = "exited".into();
        assert!(notice_for_task(&task, None, now).is_none());
    }
}
