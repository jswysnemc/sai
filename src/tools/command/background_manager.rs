//! 用户任务管理视图，独立于智能体的会话范围和完成通知消费语义。

use super::background_refresh::refresh_background_tasks;
use super::background_tasks::{is_gateway_owned_task, read_log_tail};
use super::store::BackgroundCommandStore;
use crate::{config::AppConfig, paths::SaiPaths};
use anyhow::{anyhow, Result};
use serde_json::json;

/// 【后台任务】【管理列表】返回本机非网关任务，已通知的终态记录保留到用户清理。
/// 参数: paths 为本用户应用路径，config 为运行配置；返回: JSON 任务列表。
pub(crate) async fn list_managed_background_tasks(
    paths: &SaiPaths,
    config: &AppConfig,
) -> Result<String> {
    let store = BackgroundCommandStore::new(paths.state_dir.clone());
    let tasks = refresh_background_tasks(&store, config, |task| !is_gateway_owned_task(task))
        .await?
        .into_iter()
        .filter(|task| !is_gateway_owned_task(task))
        .collect::<Vec<_>>();
    Ok(serde_json::to_string(&json!({ "tasks": tasks }))?)
}

/// 【后台任务】【管理输出】从可信任务记录读取有界日志，不确认或消费所属会话的完成通知。
/// 参数: paths/config 为环境，id 为任务标识，tail_lines 为每个流的最大行数；返回: 输出及截断标记。
pub(crate) async fn read_managed_background_output(
    paths: &SaiPaths,
    config: &AppConfig,
    id: &str,
    tail_lines: usize,
) -> Result<String> {
    let store = BackgroundCommandStore::new(paths.state_dir.clone());
    let tasks = refresh_background_tasks(&store, config, |task| {
        task.id == id && !is_gateway_owned_task(task)
    })
    .await?;
    let task = tasks
        .iter()
        .find(|task| task.id == id && !is_gateway_owned_task(task))
        .ok_or_else(|| anyhow!("background command not found: {id}"))?;
    let tail_lines = tail_lines.clamp(1, 2000);
    let max_bytes = config
        .tools
        .background_command_log_max_bytes
        .min(128 * 1024);
    let stdout = read_log_tail(&task.stdout_log, tail_lines, max_bytes)?;
    let stderr = read_log_tail(&task.stderr_log, tail_lines, max_bytes)?;
    Ok(serde_json::to_string(&json!({
        "task": task, "stdout": stdout.text, "stderr": stderr.text,
        "stdout_truncated": stdout.truncated, "stderr_truncated": stderr.truncated,
        "tail_lines": tail_lines
    }))?)
}

/// 【后台任务】【管理清理】只清理管理列表内已结束记录，保留运行任务和独立管理的网关。
/// 参数: paths/config 为环境，remove_logs 决定是否删除输出文件；返回: 实际移除标识及剩余数量。
pub(crate) async fn cleanup_managed_background_tasks(
    paths: &SaiPaths,
    config: &AppConfig,
    remove_logs: bool,
) -> Result<String> {
    let store = BackgroundCommandStore::new(paths.state_dir.clone());
    refresh_background_tasks(&store, config, |task| !is_gateway_owned_task(task)).await?;
    // 1. 【后台任务】【事务清理】在最新任务表内选择记录，不恢复刷新期间已经删除的任务
    let (removed, remaining) = store.update(|tasks| {
        let mut removed = Vec::new();
        tasks.retain(|task| {
            if task.status == "running" || is_gateway_owned_task(task) {
                return true;
            }
            removed.push(task.clone());
            false
        });
        Ok((
            removed,
            tasks
                .iter()
                .filter(|task| !is_gateway_owned_task(task))
                .count(),
        ))
    })?;
    if remove_logs {
        for task in &removed {
            let _ = std::fs::remove_file(&task.stdout_log);
            let _ = std::fs::remove_file(&task.stderr_log);
            super::exit_status::remove_exit_status_file(&task.stdout_log);
        }
    }
    Ok(serde_json::to_string(
        &json!({ "removed": removed.iter().map(|task| &task.id).collect::<Vec<_>>(), "remaining": remaining }),
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::command::BackgroundCommandTask;

    /// 【后台任务测试】【管理样本】创建跨会话完成任务和已消费记录，不操作真实进程。
    /// 参数: store 为临时存储；返回: 已保存任务。
    fn seed(store: &BackgroundCommandStore) -> Vec<BackgroundCommandTask> {
        store.init().unwrap();
        let base = BackgroundCommandTask {
            id: "one".into(),
            runtime_process_id: None,
            runtime_owner_kind: Some("session".into()),
            runtime_owner_id: Some("session-one".into()),
            runtime_process_kind: None,
            goal_id: None,
            label: "Task".into(),
            command: "echo fixture".into(),
            cwd: "/tmp".into(),
            pid: 1,
            pgid: None,
            status: "exited".into(),
            stdout_log: store.logs_dir().join("out.log").display().to_string(),
            stderr_log: store.logs_dir().join("err.log").display().to_string(),
            started_at: 0,
            updated_at: 1,
            timeout_seconds: 0,
            completion_notified: false,
        };
        let mut consumed = base.clone();
        consumed.id = "two".into();
        consumed.runtime_owner_id = Some("session-two".into());
        consumed.completion_notified = true;
        let mut gateway = base.clone();
        gateway.id = "gateway".into();
        gateway.runtime_owner_kind = Some("gateway".into());
        let tasks = vec![base, consumed, gateway];
        store.save(&tasks).unwrap();
        tasks
    }

    /// 【后台任务测试】【管理范围】显示跨会话和已通知记录，不包含网关任务。
    /// 参数: 无；返回: 无。
    #[tokio::test]
    async fn manager_keeps_finished_records_without_session_owner() {
        let dir = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(dir.path());
        let store = BackgroundCommandStore::new(paths.state_dir.clone());
        seed(&store);
        let value: serde_json::Value = serde_json::from_str(
            &list_managed_background_tasks(&paths, &AppConfig::default())
                .await
                .unwrap(),
        )
        .unwrap();
        let ids = value["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|task| task["id"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(ids, ["one", "two"]);
    }

    /// 【后台任务测试】【管理读取】日志读取有界，且不改变完成通知状态。
    /// 参数: 无；返回: 无。
    #[tokio::test]
    async fn manager_output_does_not_consume_completion_and_bounds_logs() {
        let dir = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(dir.path());
        let store = BackgroundCommandStore::new(paths.state_dir.clone());
        let tasks = seed(&store);
        std::fs::write(&tasks[0].stdout_log, "first\nsecond\nlast\n").unwrap();
        let config = AppConfig::default();
        let value: serde_json::Value = serde_json::from_str(
            &read_managed_background_output(&paths, &config, "one", 2)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(value["stdout"], "second\nlast");
        assert_eq!(value["stdout_truncated"], true);
        assert!(!store.load().unwrap()[0].completion_notified);
        assert!(
            read_managed_background_output(&paths, &config, "gateway", 2)
                .await
                .is_err()
        );
        assert!(
            read_managed_background_output(&paths, &config, "missing", 2)
                .await
                .is_err()
        );
    }

    /// 【后台任务测试】【清理范围】不删除隐藏网关或日志，只清理已结束管理记录。
    /// 参数: 无；返回: 无。
    #[tokio::test]
    async fn manager_cleanup_keeps_gateways_and_log_files() {
        let dir = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(dir.path());
        let store = BackgroundCommandStore::new(paths.state_dir.clone());
        let tasks = seed(&store);
        std::fs::write(&tasks[0].stdout_log, "kept").unwrap();
        let result: serde_json::Value = serde_json::from_str(
            &cleanup_managed_background_tasks(&paths, &AppConfig::default(), false)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(result["removed"], json!(["one", "two"]));
        assert_eq!(result["remaining"], 0);
        assert_eq!(store.load().unwrap()[0].id, "gateway");
        assert_eq!(
            std::fs::read_to_string(&tasks[0].stdout_log).unwrap(),
            "kept"
        );
    }
}
