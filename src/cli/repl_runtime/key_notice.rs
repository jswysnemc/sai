//! 输入框下方的二次按键确认与快捷键速查面板状态。

use super::composer_frame::{KeyHintContext, KeyNotice};
use super::ReplRuntime;
use std::time::{Duration, Instant};

impl ReplRuntime {
    /// 【终端】【按键提示】标记第一次 Ctrl+C，提示行在退出窗口内显示再按一次退出。
    ///
    /// 参数:
    /// - `window`: 双击退出的判定窗口
    ///
    /// 返回:
    /// - 无
    pub(in crate::cli) fn arm_exit_hint(&mut self, window: Duration) {
        self.key_notice = Some((KeyNotice::Exit, Instant::now() + window));
    }

    /// 【终端】【按键提示】有输入时第一次 Esc，提示行显示再按一次 Esc 清空输入。
    ///
    /// 参数:
    /// - `window`: 双击清空的判定窗口
    ///
    /// 返回:
    /// - 无
    pub(in crate::cli) fn arm_clear_input_hint(&mut self, window: Duration) {
        self.key_notice = Some((KeyNotice::ClearInput, Instant::now() + window));
    }

    /// 取消尚未过期的二次按键提示（例如已经完成清空或按下了其它键）。
    ///
    /// 返回:
    /// - 无
    pub(in crate::cli) fn clear_key_notice(&mut self) {
        self.key_notice = None;
    }

    /// 返回退出提示或清空提示的截止时间，供输入循环按时唤醒重绘。
    ///
    /// 返回:
    /// - 截止时间；没有提示时为空
    pub(super) fn key_notice_deadline(&self) -> Option<Instant> {
        self.key_notice.map(|(_, deadline)| deadline)
    }

    /// 汇总按键提示行需要的界面状态；过期的提示在这里清除。
    ///
    /// 返回:
    /// - 按键提示上下文
    pub(super) fn key_hint_context(&mut self) -> KeyHintContext {
        if self
            .key_notice
            .is_some_and(|(_, deadline)| deadline <= Instant::now())
        {
            self.key_notice = None;
        }
        KeyHintContext {
            streaming: self.stream_active,
            fullscreen: self.fullscreen.is_some(),
            notice: self.key_notice.map(|(notice, _)| notice),
        }
    }

    /// 【终端】【快捷键速查】空输入时切换速查面板。
    ///
    /// 参数:
    /// - `input_empty`: 当前输入是否为空；非空时 `?` 作为普通字符输入
    ///
    /// 返回:
    /// - 已切换时返回 true，调用方不再插入字符
    pub(in crate::cli) fn toggle_shortcuts(&mut self, input_empty: bool) -> bool {
        if !input_empty {
            self.shortcuts_open = false;
            return false;
        }
        self.shortcuts_open = !self.shortcuts_open;
        true
    }

    /// 【终端】【快捷键速查】按 Esc 收起速查面板。
    ///
    /// 返回:
    /// - 面板原本展开时返回 true，调用方不再按清空输入处理 Esc
    pub(in crate::cli) fn close_shortcuts(&mut self) -> bool {
        std::mem::take(&mut self.shortcuts_open)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::transcript::TranscriptRenderOptions;

    /// 构造测试运行期。
    fn runtime() -> ReplRuntime {
        ReplRuntime::new(
            1_000,
            TranscriptRenderOptions {
                reasoning_mode: crate::render::ReasoningDisplayMode::Summary,
                tool_call_mode: crate::render::ToolCallDisplayMode::Summary,
            },
        )
    }

    /// 验证二次按键提示按类型显示，过期后自动清除，新提示覆盖旧提示。
    #[test]
    fn notices_expire_and_replace_each_other() {
        let mut runtime = runtime();
        runtime.arm_exit_hint(Duration::from_secs(60));
        assert_eq!(runtime.key_hint_context().notice, Some(KeyNotice::Exit));
        assert!(runtime.pending_wait().is_some());
        runtime.arm_clear_input_hint(Duration::from_secs(60));
        assert_eq!(runtime.key_hint_context().notice, Some(KeyNotice::ClearInput));
        runtime.arm_clear_input_hint(Duration::ZERO);
        assert_eq!(runtime.key_hint_context().notice, None);
        assert!(runtime.key_notice_deadline().is_none());
        runtime.arm_exit_hint(Duration::from_secs(60));
        runtime.clear_key_notice();
        assert_eq!(runtime.key_hint_context().notice, None);
    }

    /// 验证 `?` 只在空输入时切换速查面板，Esc 收起后不再消费下一次 Esc。
    #[test]
    fn question_mark_toggles_sheet_only_on_empty_input() {
        let mut runtime = runtime();
        assert!(runtime.toggle_shortcuts(true));
        assert!(runtime.shortcuts_open);
        assert!(runtime.toggle_shortcuts(true));
        assert!(!runtime.shortcuts_open);
        assert!(!runtime.toggle_shortcuts(false));
        assert!(runtime.toggle_shortcuts(true));
        assert!(runtime.close_shortcuts());
        assert!(!runtime.close_shortcuts());
    }
}
