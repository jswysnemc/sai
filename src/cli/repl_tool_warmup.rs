use super::{build_repl_tool_registry_for_session, AgentMode, AppConfig, SaiPaths};
use crate::tools::ToolRegistry;
use anyhow::{anyhow, Result};
use std::path::PathBuf;
use std::thread::JoinHandle;

/// TUI 启动后的 MCP 工具注册表后台预热任务。
pub(super) struct ReplToolWarmup {
    mode: AgentMode,
    task: Option<JoinHandle<Result<ToolRegistry>>>,
}

impl ReplToolWarmup {
    /// 后台构建包含 MCP 动态工具的完整注册表。
    ///
    /// 参数:
    /// - `config`: 当前 TUI Agent 配置
    /// - `paths`: Sai 路径
    /// - `mode`: 启动时 Agent 模式
    /// - `session_id`: 当前会话 ID
    /// - `state_dir`: 当前会话状态目录
    ///
    /// 返回:
    /// - 可在输入循环中轮询的预热任务
    pub(super) fn start(
        config: AppConfig,
        paths: SaiPaths,
        mode: AgentMode,
        session_id: String,
        state_dir: PathBuf,
    ) -> Self {
        let task = std::thread::spawn(move || {
            build_repl_tool_registry_for_session(&config, &paths, mode, &session_id, &state_dir)
        });
        Self {
            mode,
            task: Some(task),
        }
    }

    /// 无阻塞获取已经完成的注册表。
    ///
    /// 返回:
    /// - 未完成时返回空；完成时返回启动模式和完整注册表
    pub(super) fn take_ready(&mut self) -> Option<Result<(AgentMode, ToolRegistry)>> {
        let task = self.task.as_ref()?;
        if !task.is_finished() {
            return None;
        }
        let task = self.task.take()?;
        Some(
            task.join()
                .map_err(|_| anyhow!("TUI tool registry warmup thread panicked"))
                .and_then(|registry| registry.map(|registry| (self.mode, registry))),
        )
    }

    #[cfg(test)]
    fn from_task(mode: AgentMode, task: JoinHandle<Result<ToolRegistry>>) -> Self {
        Self {
            mode,
            task: Some(task),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn polling_warmup_does_not_wait_for_mcp_discovery() {
        let task = std::thread::spawn(|| {
            std::thread::sleep(Duration::from_millis(200));
            Ok(ToolRegistry::new())
        });
        let mut warmup = ReplToolWarmup::from_task(AgentMode::Audited, task);
        let started = Instant::now();

        // 1. 未完成时立即返回空
        assert!(warmup.take_ready().is_none());
        assert!(started.elapsed() < Duration::from_millis(50));

        // 2. 轮询等待完成，避免 CI 调度抖动导致固定 sleep 不够
        let deadline = Instant::now() + Duration::from_secs(2);
        let result = loop {
            if let Some(ready) = warmup.take_ready() {
                break ready;
            }
            assert!(
                Instant::now() < deadline,
                "warmup task should finish within 2s"
            );
            std::thread::sleep(Duration::from_millis(20));
        };
        let (mode, _) = result.unwrap();
        assert_eq!(mode, AgentMode::Audited);
    }
}
