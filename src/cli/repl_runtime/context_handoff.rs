//! 上下文查看与编辑面板的区域交接及退出恢复。

use super::ReplRuntime;
use crate::control_commands::context_view::ContextOverview;
use anyhow::Result;

impl ReplRuntime {
    /// 【上下文】【编辑区域】接替输入区，按面板实际高度预留全屏区域。
    /// 参数：`height` 为卡片行数；返回：清除旧输入区后的面板位置。
    pub(in crate::cli) fn prepare_context_edit(
        &mut self,
        height: u16,
    ) -> Result<crate::question_tui::OverlayPlacement> {
        self.question_overlay_height = Some(height.max(1));
        self.pause_for_permission_prompt()?;
        // 1. 【上下文】【尺寸交接】缩放期间使用临时绘制视口，不能沿用尚未提交的旧坐标
        let viewport = self.drawing_viewport();
        let size = viewport.size();
        let panel_lines = height.clamp(1, size.rows.max(1));
        let placement = crate::question_tui::OverlayPlacement {
            cols: size.cols,
            rows: size.rows,
            anchor_y: viewport
                .composer_top()
                .min(size.rows.saturating_sub(panel_lines)),
            panel_lines,
            inline_anchor: self.fullscreen.is_none().then_some(viewport.composer_top()),
        };
        crossterm::queue!(
            self.frame,
            crossterm::style::Print("\x1b[0m"),
            crossterm::cursor::MoveTo(0, placement.anchor_y),
            crossterm::terminal::Clear(crossterm::terminal::ClearType::FromCursorDown)
        )?;
        self.last_composer_signature = None;
        self.commit_frame()?;
        Ok(placement)
    }

    /// 【上下文】【编辑面板】执行事务式编辑，结束后恢复正文、输入草稿和原视图。
    /// 参数：`paths` 为会话存储路径；返回：保存或取消结果。
    pub(in crate::cli) fn edit_context_policy(
        &mut self,
        paths: &crate::paths::SaiPaths,
    ) -> Result<String> {
        // 1. 【上下文】【编辑面板】编辑器按窗口和卡片实际行数分配区域，避免首次重复清屏
        let result = crate::config_tui::compaction::run_session(paths, |height| {
            self.prepare_context_edit(height)
        });
        let restored = self.finish_question_prompt();
        match result {
            Ok(message) => {
                restored?;
                Ok(message)
            }
            Err(error) => {
                restored?;
                Err(error)
            }
        }
    }

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
