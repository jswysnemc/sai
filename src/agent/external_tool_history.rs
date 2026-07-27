use super::AgentEvent;
use crate::state::StateStore;
use anyhow::Result;

/// 将外部内核工具事件写入 Sai 的结构化会话历史。
pub(super) struct ExternalToolHistory {
    state: StateStore,
    turn_id: String,
    next_sequence: usize,
}

impl ExternalToolHistory {
    /// 创建当前轮次的外部工具历史记录器。
    ///
    /// 参数:
    /// - `state`: 当前会话状态存储
    /// - `turn_id`: 当前轮次标识
    ///
    /// 返回:
    /// - 从已有工具数量继续编号的记录器
    pub(super) fn new(state: StateStore, turn_id: String) -> Result<Self> {
        let next_sequence = state.tool_call_count_for_turn(&turn_id)?;
        Ok(Self {
            state,
            turn_id,
            next_sequence,
        })
    }

    /// 记录一条带 provider 调用标识的工具事件。
    ///
    /// 参数:
    /// - `event`: 外部内核产生的统一事件
    ///
    /// 返回:
    /// - 持久化是否成功；非工具事件直接成功
    pub(super) fn record(&mut self, event: &AgentEvent) -> Result<()> {
        match event {
            AgentEvent::ToolCallIdentified {
                id,
                name,
                arguments,
            } => {
                self.next_sequence = self.next_sequence.saturating_add(1);
                self.state.record_tool_call_started(
                    &self.turn_id,
                    self.next_sequence,
                    id,
                    name,
                    arguments,
                )
            }
            AgentEvent::ToolResultIdentified { id, ok, output, .. } => {
                self.state.record_tool_result_completed(
                    &self.turn_id,
                    id,
                    *ok,
                    output,
                    None,
                    (!ok).then_some(output.as_str()),
                    output.chars().count(),
                )
            }
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::SaiPaths;
    use std::path::{Path, PathBuf};

    /// 构造隔离的测试路径。
    ///
    /// 参数:
    /// - `root`: 临时根目录
    ///
    /// 返回:
    /// - 会话存储所需路径
    fn test_paths(root: &Path) -> SaiPaths {
        SaiPaths {
            config_dir: PathBuf::new(),
            config_file: PathBuf::new(),
            secrets_file: PathBuf::new(),
            skills_dir: PathBuf::new(),
            data_dir: PathBuf::new(),
            cache_dir: PathBuf::new(),
            state_dir: root.join("state"),
            pictures_dir: PathBuf::new(),
            fish_hook_file: PathBuf::new(),
            bash_hook_file: PathBuf::new(),
            zsh_hook_file: PathBuf::new(),
            powershell_hook_file: PathBuf::new(),
        }
    }

    /// 外部工具事件在轮次完成后仍应出现在结构化时间线中。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn persists_identified_tools_for_history_rendering() {
        let temp = tempfile::tempdir().unwrap();
        let state = StateStore::new(&test_paths(temp.path())).unwrap();
        state.start_turn("turn-1", "检查文件").unwrap();
        let mut history = ExternalToolHistory::new(state.clone(), "turn-1".to_string()).unwrap();

        history
            .record(&AgentEvent::ToolCallIdentified {
                id: "call-1".to_string(),
                name: "Read file".to_string(),
                arguments: r#"{"path":"README.md"}"#.to_string(),
            })
            .unwrap();
        history
            .record(&AgentEvent::ToolResultIdentified {
                id: "call-1".to_string(),
                name: "Read file".to_string(),
                ok: true,
                output: "content".to_string(),
            })
            .unwrap();
        state.complete_turn("turn-1", "完成", None).unwrap();

        let timeline = state.session_timeline(10).unwrap();
        assert_eq!(timeline[0].tools.len(), 1);
        assert_eq!(timeline[0].tools[0].id, "call-1");
        assert_eq!(timeline[0].tools[0].name, "Read file");
        assert_eq!(timeline[0].tools[0].output, "content");
    }
}
