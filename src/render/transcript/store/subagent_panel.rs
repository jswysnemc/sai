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
    /// 子智能体类型，并发多个时用来区分
    pub(crate) agent_type: Option<String>,
    /// 已执行步数与预算
    pub(crate) progress: Option<(usize, usize)>,
    /// 运行时长（秒）
    pub(crate) elapsed_seconds: Option<u64>,
}

impl TranscriptStore {
    /// 枚举底部 agent 面板可切换的子智能体。
    ///
    /// 同一个子智能体的每次工具调用（start / wait / send / result）都会
    /// 产生一个 transcript cell，这里按后台 ID 去重，只保留一个条目并
    /// 随最新 cell 更新状态。
    ///
    /// 终态条目同样保留：并发跑完一批子智能体后，用户要能在一处看到
    /// 每个任务的结果与最终用量，而不是让它们跑完就从列表里消失。
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
            let entry = SubagentOverviewEntry {
                cell_index: index,
                label: overview.label,
                status: overview.status,
                running: overview.running,
                viewing,
                detail: overview.detail,
                tokens: overview.tokens,
                agent_type: overview.agent_type,
                progress: overview.progress,
                elapsed_seconds: overview.elapsed_seconds,
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
        trim_finished_entries(&mut entries);
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

/// 面板保留的终态条目上限。
///
/// transcript 里的子智能体 cell 会随会话累积，全量保留会让底栏无限长。
/// 运行中、待命与正在查看的条目不受此限制——它们正是用户此刻要盯的。
const FINISHED_OVERVIEW_LIMIT: usize = 8;

/// 丢弃超出上限的终态条目，保留最近结束的那些。
///
/// 参数:
/// - `entries`: 已按 transcript 顺序排列的概览条目
///
/// 返回:
/// - 无
fn trim_finished_entries(entries: &mut Vec<SubagentOverviewEntry>) {
    let mut finished: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| !entry.running && !entry.viewing && entry.status != "idle")
        .map(|(index, _)| index)
        .collect();
    let excess = finished.len().saturating_sub(FINISHED_OVERVIEW_LIMIT);
    if excess == 0 {
        return;
    }
    finished.truncate(excess);
    // 下标升序，从后往前删避免删除后位移
    for &index in finished.iter().rev() {
        entries.remove(index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造测试概览条目。
    fn entry(label: &str, running: bool, status: &'static str) -> SubagentOverviewEntry {
        SubagentOverviewEntry {
            cell_index: 0,
            label: label.to_string(),
            status,
            running,
            viewing: false,
            detail: None,
            tokens: None,
            agent_type: None,
            progress: None,
            elapsed_seconds: None,
        }
    }

    /// 终态条目在上限内全部保留。
    #[test]
    fn finished_entries_are_kept_within_the_limit() {
        let mut entries: Vec<_> = (0..FINISHED_OVERVIEW_LIMIT)
            .map(|index| entry(&format!("任务{index}"), false, "ok"))
            .collect();
        trim_finished_entries(&mut entries);
        assert_eq!(entries.len(), FINISHED_OVERVIEW_LIMIT);
    }

    /// 超出上限时丢弃最早结束的那些，保留最近的。
    #[test]
    fn oldest_finished_entries_are_dropped_first() {
        let total = FINISHED_OVERVIEW_LIMIT + 2;
        let mut entries: Vec<_> = (0..total)
            .map(|index| entry(&format!("任务{index}"), false, "ok"))
            .collect();
        trim_finished_entries(&mut entries);
        assert_eq!(entries.len(), FINISHED_OVERVIEW_LIMIT);
        let labels: Vec<&str> = entries.iter().map(|item| item.label.as_str()).collect();
        assert!(!labels.contains(&"任务0"), "{labels:?}");
        assert!(!labels.contains(&"任务1"), "{labels:?}");
        assert!(
            labels.contains(&format!("任务{}", total - 1).as_str()),
            "{labels:?}"
        );
    }

    /// 运行中、待命与正在查看的条目永远不被裁剪。
    #[test]
    fn running_and_idle_entries_are_never_trimmed() {
        let total = FINISHED_OVERVIEW_LIMIT + 4;
        let mut entries: Vec<_> = (0..total)
            .map(|index| {
                if index % 2 == 0 {
                    entry(&format!("任务{index}"), true, "run")
                } else {
                    entry(&format!("任务{index}"), false, "idle")
                }
            })
            .collect();
        trim_finished_entries(&mut entries);
        assert_eq!(entries.len(), total);
    }
}
