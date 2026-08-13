use super::viewport::TerminalSize;
use std::time::{Duration, Instant};

pub(super) const REFLOW_DEBOUNCE: Duration = Duration::from_millis(75);

/// REPL resize reflow 的观察与流式收敛状态。
pub(super) struct ReflowState {
    last_observed: Option<TerminalSize>,
    last_reflowed: Option<TerminalSize>,
    pending_until: Option<Instant>,
    pending_size: Option<TerminalSize>,
    ran_during_stream: bool,
    resize_requested_during_stream: bool,
}

impl ReflowState {
    /// 创建空的 resize reflow 状态。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 初始 reflow 状态
    pub(super) fn new() -> Self {
        Self {
            last_observed: None,
            last_reflowed: None,
            pending_until: None,
            pending_size: None,
            ran_during_stream: false,
            resize_requested_during_stream: false,
        }
    }

    /// 记录一次终端尺寸观察，并在需要时安排 trailing debounce。
    ///
    /// 只有尺寸真正变化才重设 deadline。尺寸未变的重复观察来自 32ms 动效帧与
    /// 25ms 主循环 tick 的一致性巡检，若一并推后 deadline，75ms 的 debounce
    /// 永远等不到窗口，reflow 不会执行，viewport 也就一直重锚不了。
    ///
    /// 参数:
    /// - `size`: 本次观察到的终端尺寸
    /// - `streaming`: 是否处于流式输出
    ///
    /// 返回:
    /// - 是否已安排 reflow
    pub(super) fn observe(&mut self, size: TerminalSize, streaming: bool) -> bool {
        let previous = self.last_observed.replace(size);
        if previous.is_none() {
            self.last_reflowed = Some(size);
            return false;
        }
        if previous == Some(size) && self.last_reflowed == Some(size) {
            return false;
        }
        // 1. 尺寸仍在变化说明用户还在拖拽，重新起算 debounce
        // 2. 尺寸已稳定且已有排期时保留原 deadline，让它按时到期
        if previous != Some(size) || self.pending_until.is_none() {
            self.pending_until = Some(Instant::now() + REFLOW_DEBOUNCE);
        }
        self.pending_size = Some(size);
        if streaming {
            self.resize_requested_during_stream = true;
        }
        true
    }

    /// 立即安排一次 source-backed 重放。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    pub(super) fn schedule_immediate(&mut self) {
        self.pending_size = None;
        self.pending_until = Some(Instant::now());
    }

    /// 判断当前 pending reflow 是否到期。
    ///
    /// 参数:
    /// - `now`: 当前时间
    ///
    /// 返回:
    /// - 是否可以开始重放
    pub(super) fn is_due(&self, now: Instant) -> bool {
        self.pending_until.is_some_and(|deadline| now >= deadline)
    }

    /// 返回下一次 pending deadline。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 仍待执行时的 deadline
    pub(super) fn pending_until(&self) -> Option<Instant> {
        self.pending_until
    }

    /// 清除当前 pending reflow。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    pub(super) fn clear_pending(&mut self) {
        self.pending_until = None;
        self.pending_size = None;
    }

    /// 记录已在指定尺寸完成 source-backed 重放。
    ///
    /// 参数:
    /// - `size`: 实际参与重放的终端尺寸
    /// - `streaming`: 是否处于流式输出
    ///
    /// 返回:
    /// - 无
    pub(super) fn mark_reflowed(&mut self, size: TerminalSize, streaming: bool) {
        self.last_reflowed = Some(size);
        if streaming {
            self.ran_during_stream = true;
        }
    }

    /// 取出流式收敛后必须补偿重放的标记。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 是否需要收敛后的强制重放
    pub(super) fn take_stream_finish_reflow_needed(&mut self) -> bool {
        let needed = self.ran_during_stream || self.resize_requested_during_stream;
        self.ran_during_stream = false;
        self.resize_requested_during_stream = false;
        needed
    }

    /// 重置全部 reflow 观察与流式标记。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    pub(super) fn clear(&mut self) {
        *self = Self::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造测试用终端尺寸。
    ///
    /// 参数:
    /// - `cols`: 列数
    ///
    /// 返回:
    /// - 终端尺寸
    fn size(cols: u16) -> TerminalSize {
        TerminalSize { cols, rows: 24 }
    }

    /// 【TUI】【resize 动效】验证尺寸稳定后的重复观察不推后 debounce。
    ///
    /// 动效帧每 32ms 巡检一次尺寸，若每次都重设 75ms 的 deadline，
    /// reflow 永远不到期，屏幕停在 resize 前的画面。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn repeated_same_size_observation_keeps_deadline() {
        let mut state = ReflowState::new();
        state.observe(size(80), false);
        state.observe(size(100), false);
        let scheduled = state.pending_until().expect("resize must schedule reflow");

        // 1. 模拟动效帧反复巡检同一尺寸
        for _ in 0..8 {
            state.observe(size(100), false);
        }

        assert_eq!(state.pending_until(), Some(scheduled));
    }

    /// 【TUI】【resize 动效】验证持续拖拽仍然重新起算 debounce。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn size_change_extends_deadline() {
        let mut state = ReflowState::new();
        state.observe(size(80), false);
        state.observe(size(100), false);
        let scheduled = state.pending_until().expect("resize must schedule reflow");

        state.observe(size(120), false);

        assert!(state.pending_until().expect("still pending") > scheduled);
    }

    /// 【TUI】【resize 动效】验证到期重放后同尺寸观察不再排期。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn observation_after_reflow_stops_scheduling() {
        let mut state = ReflowState::new();
        state.observe(size(80), false);
        state.observe(size(100), false);
        state.clear_pending();
        state.mark_reflowed(size(100), false);

        assert!(!state.observe(size(100), false));
        assert!(state.pending_until().is_none());
    }
}
