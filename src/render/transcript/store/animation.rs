use super::*;
use crate::render::activity_animation::{activity_frame_at, activity_started_at};
use crate::render::transcript::tool_cell::ToolCell;

impl TranscriptStore {
    /// 是否存在需要按帧重绘的进行中工具卡。
    fn has_pending_tool_cells(&self) -> bool {
        self.cells.iter().any(|cell| match cell {
            HistoryCell::Tool(ToolCell::Invocation(view)) => {
                view.outcome.is_none() || view.is_running_background()
            }
            HistoryCell::Tool(ToolCell::CompactionStarted { .. }) => true,
            HistoryCell::Diff(diff) => diff.is_pending(),
            HistoryCell::Tool(ToolCell::Subagent(cell)) => cell.is_active(),
            _ => false,
        })
    }

    /// 第一张进行中工具卡的下标，供动效把脏水位提前到这些行。
    fn first_pending_tool_index(&self) -> Option<usize> {
        self.cells.iter().enumerate().find_map(|(index, cell)| {
            let pending = match cell {
                HistoryCell::Tool(ToolCell::Invocation(view)) => {
                    view.outcome.is_none() || view.is_running_background()
                }
                HistoryCell::Tool(ToolCell::CompactionStarted { .. }) => true,
                HistoryCell::Diff(diff) => diff.is_pending(),
                HistoryCell::Tool(ToolCell::Subagent(cell)) => cell.is_active(),
                _ => false,
            };
            pending.then_some(index)
        })
    }

    /// 维持 live 动效的计时并判断是否仍需刷新。
    ///
    /// 帧号由共享时钟换算；首次出现动效时绑定进程起点，
    /// 切换状态或会话不会重置扫描位置，调用频率也不影响动效速度。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 是否存在需要刷新的 live 动效
    pub(crate) fn advance_live_animation(&mut self) -> bool {
        let has_reasoning = self
            .live_tail
            .as_ref()
            .is_some_and(|tail| tail.kind == ChatStreamKind::Reasoning && !tail.source.is_empty());
        let has_work_status = self.work_status.is_some();
        // 子智能体视图在主 agent 空闲时仍需刷新，否则 Working 扫光会冻结成静态图；
        // 主视图下有后台子智能体运行时同样推进帧，驱动底部面板的流光与实时统计
        if !has_reasoning
            && !has_work_status
            && !self.has_pending_tool_cells()
            && !self.viewing_running_subagent()
            && !self.has_running_subagents()
        {
            return false;
        }
        self.live_animation_started
            .get_or_insert_with(activity_started_at);
        if let Some(index) = self.first_pending_tool_index() {
            self.mark_dirty(index);
        }
        true
    }

    /// 返回当前应当渲染的 live 动效帧序号。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 计时起点至今换算出的帧序号；动效尚未开始时返回 0
    pub(crate) fn live_animation_frame(&self) -> usize {
        self.live_animation_started
            .map(|started| activity_frame_at(started.elapsed()))
            .unwrap_or_default()
    }

    /// 【终端】【状态动效测试】指定本窗口已经经过的动画时长，不改动进程共享时钟。
    ///
    /// 参数:
    /// - `elapsed`: 需要模拟经过的时长
    ///
    /// 返回:
    /// - 无
    #[cfg(test)]
    pub(crate) fn set_live_animation_elapsed_for_test(&mut self, elapsed: std::time::Duration) {
        let started = Instant::now()
            .checked_sub(elapsed)
            .unwrap_or_else(Instant::now);
        self.live_animation_started = Some(started);
    }

    /// 判断当前是否停留在仍在运行的子智能体视图。
    ///
    /// 返回:
    /// - 子智能体视图且该子智能体运行中时返回 true
    pub(crate) fn viewing_running_subagent(&self) -> bool {
        let TranscriptView::Subagent { id, .. } = &self.view else {
            return false;
        };
        crate::tools::subagent_state::subagent_snapshot(id)
            .is_ok_and(|snapshot| snapshot.status == "running")
    }

    /// 判断 transcript 中是否有仍在更新的后台子智能体。
    ///
    /// 返回:
    /// - 需要定时重绘时返回 true
    pub(crate) fn has_running_subagents(&self) -> bool {
        self.cells.iter().any(|cell| {
            matches!(
                cell,
                HistoryCell::Tool(ToolCell::Subagent(subagent)) if subagent.has_live_updates()
            )
        })
    }

    /// 返回 transcript 中子智能体状态和时间线签名。
    ///
    /// 返回:
    /// - 按 cell 顺序组织的状态签名
    pub(crate) fn subagent_signature(&self) -> Vec<(String, String, u64, u64)> {
        self.cells
            .iter()
            .filter_map(|cell| match cell {
                HistoryCell::Tool(ToolCell::Subagent(subagent)) => subagent.state_signature(),
                _ => None,
            })
            .collect()
    }
}
