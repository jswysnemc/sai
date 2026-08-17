use super::{TranscriptStore, TranscriptView};
use crate::render::transcript::cell::HistoryCell;
use crate::render::transcript::tool_cell::ToolCell;
use std::collections::HashMap;

/// 底部 agent 面板展示的子智能体概览条目。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SubagentOverviewEntry {
    /// transcript 中对应 cell 的下标
    pub(crate) cell_index: usize,
    /// 展示名称（快照描述或参数摘要）
    pub(crate) label: String,
    /// 状态键：ok / err / run / idle
    pub(crate) status: &'static str,
    /// 是否仍在运行
    pub(crate) running: bool,
    /// 是否为当前正在查看的子智能体视图
    pub(crate) viewing: bool,
    /// 存活条目的实时阶段（工具进度 / Token 统计 / 待命提示）
    pub(crate) detail: Option<String>,
    /// 累计消耗的 token 估算，供面板左栏跳动
    pub(crate) tokens: Option<u64>,
}

impl TranscriptStore {
    /// 枚举底部 agent 面板可切换的子智能体（存活的与正在查看的）。
    ///
    /// 同一个子智能体的每次工具调用（start / wait / send / result）都会
    /// 产生一个 transcript cell，这里按后台 ID 去重，只保留一个条目并
    /// 随最新 cell 更新状态。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 子智能体概览列表；无可切换条目时为空
    pub(crate) fn subagent_overview(&self) -> Vec<SubagentOverviewEntry> {
        let viewing_id = self.viewing_subagent_id().map(str::to_string);
        let mut entries: Vec<SubagentOverviewEntry> = Vec::new();
        let mut position_by_id: HashMap<String, usize> = HashMap::new();
        for (index, cell) in self.cells.iter().enumerate() {
            let HistoryCell::Tool(ToolCell::Subagent(subagent)) = cell else {
                continue;
            };
            let overview = subagent.overview();
            let viewing = viewing_id
                .as_deref()
                .is_some_and(|id| subagent.subagent_id().is_some_and(|cell_id| cell_id == id));
            // 存活（运行中/待命中）的出现在面板；正在查看的保留返回路径
            let alive = overview.running || overview.status == "idle";
            if !alive && !viewing {
                continue;
            }
            let entry = SubagentOverviewEntry {
                cell_index: index,
                label: overview.label,
                status: overview.status,
                running: overview.running,
                viewing,
                detail: overview.detail,
                tokens: overview.tokens,
            };
            if let Some(id) = subagent.subagent_id() {
                if let Some(&position) = position_by_id.get(id) {
                    // 同一子智能体的后续调用只刷新已有条目，保持首次出现的顺序
                    entries[position] = entry;
                    continue;
                }
                position_by_id.insert(id.to_string(), entries.len());
            }
            entries.push(entry);
        }
        entries
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
        let label = subagent.overview().label;
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
