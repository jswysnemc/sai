use super::{TranscriptStore, TranscriptView};
use crate::render::transcript::cell::HistoryCell;
use crate::render::transcript::tool_cell::ToolCell;

/// 底部 agent 面板展示的子智能体概览条目。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SubagentOverviewEntry {
    /// transcript 中对应 cell 的下标
    pub(crate) cell_index: usize,
    /// 展示名称（快照描述或参数摘要）
    pub(crate) label: String,
    /// 状态键：ok / err / run
    pub(crate) status: &'static str,
    /// 是否仍在运行
    pub(crate) running: bool,
    /// 是否为当前正在查看的子智能体视图
    pub(crate) viewing: bool,
}

impl TranscriptStore {
    /// 枚举底部 agent 面板可切换的子智能体（运行中的与正在查看的）。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 子智能体概览列表；无可切换条目时为空
    pub(crate) fn subagent_overview(&self) -> Vec<SubagentOverviewEntry> {
        let viewing_id = self.viewing_subagent_id().map(str::to_string);
        self.cells
            .iter()
            .enumerate()
            .filter_map(|(index, cell)| match cell {
                HistoryCell::Tool(ToolCell::Subagent(subagent)) => {
                    let (label, status, running) = subagent.overview();
                    let viewing = viewing_id.as_deref().is_some_and(|id| {
                        subagent.subagent_id().is_some_and(|cell_id| cell_id == id)
                    });
                    // 子智能体只在运行时出现在面板；正在查看的保留返回路径
                    if !running && !viewing {
                        return None;
                    }
                    Some(SubagentOverviewEntry {
                        cell_index: index,
                        label,
                        status,
                        running,
                        viewing,
                    })
                }
                _ => None,
            })
            .collect()
    }

    /// 切换到指定子智能体的会话视图。
    ///
    /// 参数:
    /// - `cell_index`: transcript cell 下标
    ///
    /// 返回:
    /// - 是否成功切换（子智能体尚未绑定后台 ID 时失败）
    pub(crate) fn enter_subagent_view(&mut self, cell_index: usize) -> bool {
        let Some(HistoryCell::Tool(ToolCell::Subagent(subagent))) = self.cells.get(cell_index)
        else {
            return false;
        };
        let Some(id) = subagent.subagent_id() else {
            return false;
        };
        let (label, _, _) = subagent.overview();
        self.view = TranscriptView::Subagent {
            id: id.to_string(),
            label,
        };
        true
    }

    /// 返回主 agent 会话视图。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 视图是否发生变化
    pub(crate) fn exit_subagent_view(&mut self) -> bool {
        if self.view == TranscriptView::Main {
            return false;
        }
        self.view = TranscriptView::Main;
        true
    }

    /// 返回当前正在查看的子智能体 ID。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 处于子智能体视图时返回其 ID
    pub(crate) fn viewing_subagent_id(&self) -> Option<&str> {
        match &self.view {
            TranscriptView::Subagent { id, .. } => Some(id),
            TranscriptView::Main => None,
        }
    }
}
