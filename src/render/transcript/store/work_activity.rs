use super::TranscriptStore;
use crate::render::work_status::WorkStatus;
use std::time::Instant;

impl TranscriptStore {
    /// 更新当前单轮工作状态。
    ///
    /// 参数:
    /// - `status`: 新工作状态
    ///
    /// 返回:
    /// - 状态是否发生变化
    pub(crate) fn set_work_status(&mut self, status: WorkStatus) -> bool {
        if self.work_status == Some(status) {
            return false;
        }
        self.work_status = Some(status);
        // 本轮首次进入工作态时开始计时，后续状态切换不重置
        if self.work_status_started.is_none() {
            self.work_status_started = Some(Instant::now());
        }
        true
    }

    /// 清除当前单轮工作状态。
    ///
    /// 返回:
    /// - 是否清除了状态
    pub(crate) fn clear_work_status(&mut self) -> bool {
        self.work_status_started = None;
        self.work_status.take().is_some()
    }
}
