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
        self.pending_size = Some(size);
        self.pending_until = Some(Instant::now() + REFLOW_DEBOUNCE);
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
