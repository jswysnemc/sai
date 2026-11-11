//! 【终端】【底栏动作】处理单击入口，保留原有拖选复制和输入草稿。

use super::{ReplRuntime, StreamComposerDraft};
use crate::cli::mouse_menu::ClickTracker;
use crate::cli::repl_chrome::FooterAction;
use anyhow::Result;
use crossterm::event::{Event, MouseEventKind};

#[derive(Default)]
pub(super) struct FooterMouse {
    click: ClickTracker<FooterAction>,
    draft: Option<StreamComposerDraft>,
}

impl ReplRuntime {
    /// 【终端】【底栏目标】参数为屏幕坐标；返回最近绘制帧中的可点击字段。
    pub(super) fn footer_target_at(&self, column: u16, row: u16) -> Option<FooterAction> {
        self.last_composer_signature
            .as_ref()?
            .footer_actions
            .iter()
            .find(|(candidate, columns, _)| {
                *candidate == row && columns.contains(&usize::from(column))
            })
            .map(|(_, _, action)| *action)
    }
    /// 【终端】【底栏单击】参数为终端事件；返回完整可见字段上的有效单击，拖动及缩放不触发。
    pub(in crate::cli) fn footer_action(&mut self, event: &Event) -> Option<FooterAction> {
        let Event::Mouse(mouse) = event else {
            self.footer_mouse.click.clear();
            return None;
        };
        if self.background_panel_active() || self.image_viewer_active() {
            self.footer_mouse.click.clear();
            return None;
        }
        let target = self.footer_target_at(mouse.column, mouse.row);
        let action = self.footer_mouse.click.update(*mouse, target)?;
        self.composer_selection.clear();
        Some(action)
    }

    /// 【终端】【底栏草稿】参数为完整编辑状态；暂存到面板关闭后的下一次输入，返回无。
    pub(in crate::cli) fn save_footer_draft(&mut self, draft: StreamComposerDraft) {
        self.footer_mouse.draft = Some(draft);
    }

    /// 【终端】【底栏恢复】无参数；取回且清除面板打开前的输入、附件和光标。
    pub(in crate::cli) fn take_footer_draft(&mut self) -> Option<StreamComposerDraft> {
        self.footer_mouse.draft.take()
    }

    /// 【终端】【内联鼠标】参数为事件；非全屏时复用输入与底栏拖选，返回是否消费事件。
    pub(in crate::cli) fn handle_inline_composer_mouse(&mut self, event: &Event) -> Result<bool> {
        if self.fullscreen_active() {
            return Ok(false);
        }
        let Event::Mouse(mouse) = event else {
            return Ok(false);
        };
        let hovered = if mouse.kind == MouseEventKind::Moved {
            let before = self
                .panel_pointer
                .and_then(|(column, row)| self.footer_target_at(column, row));
            let target = self.footer_target_at(mouse.column, mouse.row);
            self.panel_pointer = target.map(|_| (mouse.column, mouse.row));
            before != target
        } else {
            false
        };
        let effect = self.composer_selection.handle(
            *mouse,
            self.last_composer_signature
                .as_ref()
                .and_then(|signature| signature.input_geometry.as_ref()),
            self.last_composer_signature
                .as_ref()
                .and_then(|signature| signature.footer_geometry.as_ref()),
        );
        if let Some(text) = effect.copy {
            use std::io::Write;
            write!(
                self.frame,
                "{}",
                super::fullscreen::selection::osc52_copy(&text)
            )?;
            self.commit_frame()?;
        }
        Ok(effect.handled || hovered)
    }
}
