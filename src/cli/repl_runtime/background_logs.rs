use super::ReplRuntime;
use crate::paths::SaiPaths;
use crate::tools::command::{background_log_snapshot, process_exists, BackgroundCommandStore};
use anyhow::Result;
use std::time::{Duration, Instant};

/// 当前会话的日志读取上下文；刷新按秒节流，与动效帧率分开。
#[derive(Default)]
pub(super) struct BackgroundLogWatcher {
    source: Option<(SaiPaths, String)>,
    next_read: Option<Instant>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 后台日志无需主 Agent 唤醒或用户按键即可刷新，记录清理后停止轮询。
    #[test]
    fn background_command_idle_refresh_has_an_independent_wakeup() {
        let temp = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(temp.path());
        let mut runtime = ReplRuntime::new(
            100,
            crate::render::transcript::TranscriptRenderOptions {
                reasoning_mode: crate::render::ReasoningDisplayMode::Summary,
                tool_call_mode: crate::render::ToolCallDisplayMode::Full,
            },
        );
        runtime.bind_background_session(&paths, "test-session");
        runtime.transcript.push_tool_call(
            "background_command".into(),
            r#"{"action":"start","command":"build"}"#.into(),
        );
        runtime.transcript.push_tool_result("background_command".into(), true,
            r#"{"task":{"id":"test-task","status":"running","command":"build"},"stdout":"last log"}"#.into());
        assert!(runtime.pending_wait().is_some());
        assert!(runtime.tick_background_logs().unwrap());
        assert!(runtime.transcript.running_background_task_ids().is_empty());
        assert!(runtime.pending_wait().is_none());
        assert!(runtime
            .transcript
            .expandable_blocks()
            .iter()
            .any(|block| block.body.contains("last log")));
    }
}

impl ReplRuntime {
    /// 【终端】【后台日志】绑定当前会话，切换会话时重置刷新时刻。
    ///
    /// 参数: `paths` 为可信任务存储路径，`session_id` 为当前会话标识
    /// 返回: 无
    pub(in crate::cli) fn bind_background_session(&mut self, paths: &SaiPaths, session_id: &str) {
        if self
            .background_logs
            .source
            .as_ref()
            .is_some_and(|(_, id)| id == session_id)
        {
            return;
        }
        self.background_logs.source = Some((paths.clone(), session_id.to_string()));
        self.background_logs.next_read = None;
    }

    /// 【终端】【后台日志】只更新当前会话中仍在运行的启动卡片。
    ///
    /// 参数: 无
    /// 返回: 是否存在可见状态或日志变化；只读文件，不消费主 Agent 的进展提醒
    pub(super) fn tick_background_logs(&mut self) -> Result<bool> {
        let Some((paths, session_id)) = self.background_logs.source.as_ref() else {
            return Ok(false);
        };
        let now = Instant::now();
        if self
            .background_logs
            .next_read
            .is_some_and(|next| now < next)
        {
            return Ok(false);
        }
        self.background_logs.next_read = Some(now + Duration::from_secs(1));
        let ids = self.transcript.running_background_task_ids();
        if ids.is_empty() {
            return Ok(false);
        }
        let tasks = BackgroundCommandStore::new(paths.state_dir.clone()).load()?;
        let mut changed = false;
        for id in ids.iter().filter(|id| {
            !tasks
                .iter()
                .any(|task| task.owned_by_session(session_id) && &task.id == *id)
        }) {
            // 1. 记录清理后结束跟踪，保留最后读取的命令和日志，不推断进程结果
            changed |= self.transcript.update_background_logs(
                id,
                &serde_json::json!({
                    "task": {"status": "untracked"}
                }),
            );
        }
        for mut task in tasks
            .into_iter()
            .filter(|task| task.owned_by_session(session_id) && ids.contains(&task.id))
        {
            // 2. 卡片可以观察退出，任务状态写入仍由命令工具与外部事件监听负责
            if task.status == "running" && !process_exists(task.pid) {
                task.status = "exited".to_string();
            }
            // 3. 日志只从可信任务表取路径，不读取工具结果中任意指定的文件
            if let Ok(snapshot) = background_log_snapshot(&task) {
                changed |= self.transcript.update_background_logs(&task.id, &snapshot);
            }
        }
        Ok(changed)
    }
}
