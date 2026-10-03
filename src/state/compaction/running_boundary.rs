use super::{selector::MIN_RUNNING_CALLS_TO_AUTO_COMPACT, CompactionRequest};
use crate::state::tool_history::{aligned_compaction_prefix, load_tool_exchanges_for_turn};
use crate::state::StateStore;
use anyhow::Result;

impl StateStore {
    /// 【上下文】【压缩范围选择】在摘要请求生成前对齐子轮，保护最近工具并拒绝无进展压缩
    /// 参数: request 为累计覆盖范围，already_compacted 为旧边界，force 为手动或溢出强制请求；返回可执行范围或空值
    pub(super) fn align_running_compaction(
        &self,
        request: Option<CompactionRequest>,
        already_compacted: usize,
        force: bool,
    ) -> Result<Option<CompactionRequest>> {
        let Some(mut request) = request else {
            return Ok(None);
        };
        if let Some(running) = request.running_turn.as_mut() {
            let exchanges =
                load_tool_exchanges_for_turn(&self.conv_db, &self.session_id, &running.turn_id)?;
            running.compacted_calls =
                aligned_compaction_prefix(&exchanges, running.compacted_calls);
            let newly_covered = running.compacted_calls.saturating_sub(already_compacted);
            // 1. 【上下文】【压缩范围选择】对齐后的收益重新满足自动阈值，不能重复压缩相同边界
            if newly_covered == 0 || (!force && newly_covered < MIN_RUNNING_CALLS_TO_AUTO_COMPACT) {
                request.running_turn = None;
            }
        }
        Ok(request.has_content().then_some(request))
    }
}
