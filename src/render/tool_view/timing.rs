use super::ToolView;
use std::time::{Duration, Instant};

/// 只记录实时工具的生命周期，历史恢复不从重绘时间推导执行耗时。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ToolTiming {
    started: Instant,
    finished: Option<Duration>,
}

impl ToolTiming {
    /// 【终端】【工具计时】冻结结束时耗时，后续重绘不再增长。
    /// 参数: 无；返回: 无。
    pub(super) fn finish(&mut self) {
        self.finished.get_or_insert_with(|| self.started.elapsed());
    }
}

impl ToolView {
    /// 【终端】【工具计时】启动实时工具计时，不影响模型参数和历史数据。
    /// 参数: 无；返回: 无。
    pub(crate) fn start_timer(&mut self) {
        self.timing = Some(ToolTiming {
            started: Instant::now(),
            finished: None,
        });
    }

    /// 【终端】【工具计时】历史没有原始耗时时保持未知，不显示恢复操作的耗时。
    /// 参数: 无；返回: 无。
    pub(crate) fn clear_timer(&mut self) {
        self.timing = None;
    }

    /// 【终端】【工具耗时】取得实时或已冻结耗时，未计时的历史记录返回空。
    /// 参数: 无；返回: 生命周期耗时。
    pub(crate) fn elapsed(&self) -> Option<Duration> {
        self.timing
            .as_ref()
            .map(|timing| timing.finished.unwrap_or_else(|| timing.started.elapsed()))
    }
}
