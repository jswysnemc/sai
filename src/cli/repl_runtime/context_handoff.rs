//! 上下文只读面板的区域交接与退出恢复。

use super::ReplRuntime;
use crate::control_commands::context_view::ContextOverview;
use anyhow::Result;

impl ReplRuntime {
    /// 【上下文】【区域交接】接替输入区展示上下文，退出或报错后恢复正文与草稿。
    /// 参数：`view` 为不包含会话秘密的上下文快照；返回：显示和恢复结果。
    pub(in crate::cli) fn show_context_panel(&mut self, view: &ContextOverview) -> Result<()> {
        // 1. 【上下文】【区域交接】复用独占面板的高度分配，不进入新的全屏视图
        self.prepare_question_prompt()?;
        let placement = self.question_overlay_placement();
        let result = crate::cli::context_panel::show(view, placement, || {
            self.redraw()?;
            self.prepare_question_prompt()?;
            Ok(self.question_overlay_placement())
        });
        // 2. 【上下文】【区域交接】即使读取按键失败也撤下面板并恢复输入区域
        let restored = self.finish_question_prompt();
        result.and(restored)
    }
}
