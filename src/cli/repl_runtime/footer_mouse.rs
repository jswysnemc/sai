//! 【终端】【底栏动作】处理单击入口、内联沉底面板鼠标交互，并保留原有拖选复制和输入草稿。

use super::panel_mouse::PanelAction;
use super::{ReplRuntime, StreamComposerDraft};
use crate::cli::mouse_menu::ClickTracker;
use crate::cli::repl_chrome::FooterAction;
use anyhow::Result;
use crossterm::event::{Event, MouseButton, MouseEventKind};

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

    /// 【终端】【内联鼠标】参数为事件；非全屏时处理沉底面板点击、队列滚轮与输入框拖选，返回是否消费事件。
    pub(in crate::cli) fn handle_inline_composer_mouse(&mut self, event: &Event) -> Result<bool> {
        if self.fullscreen_active() {
            return Ok(false);
        }
        let Event::Mouse(mouse) = event else {
            return Ok(false);
        };
        // 1. 【终端】【队列滚轮】内联模式下指针停在队列条目上时支持滚轮上下切换选中项
        if matches!(
            mouse.kind,
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
        ) && matches!(
            self.panel_target_at(mouse.column, mouse.row),
            Some(PanelAction::QueueItem(_))
        ) {
            let len = self.queue_len();
            let delta = if mouse.kind == MouseEventKind::ScrollUp {
                -1
            } else {
                1
            };
            self.queue_panel.select(
                self.queue_panel
                    .selected()
                    .saturating_add_signed(delta)
                    .min(len.saturating_sub(1)),
            );
            self.clamp_queue_panel();
            return Ok(true);
        }
        // 2. 【终端】【悬停反馈】仅在底栏动作或沉底面板命中目标变化时触发重绘
        let hovered = if mouse.kind == MouseEventKind::Moved {
            let before_footer = self
                .panel_pointer
                .and_then(|(column, row)| self.footer_target_at(column, row));
            let before_panel = self
                .panel_pointer
                .and_then(|(column, row)| self.panel_target_at(column, row));
            let footer_hit = self.footer_target_at(mouse.column, mouse.row);
            let panel_hit = self.panel_target_at(mouse.column, mouse.row);
            self.panel_pointer =
                (footer_hit.is_some() || panel_hit.is_some()).then_some((mouse.column, mouse.row));
            before_footer != footer_hit || before_panel != panel_hit
        } else {
            false
        };
        // 3. 【终端】【输入拖选】先处理输入框与底栏文本选择，点击输入框时退出队列管理态
        let effect = self.composer_selection.handle(
            *mouse,
            self.last_composer_signature
                .as_ref()
                .and_then(|signature| signature.input_geometry.as_ref()),
            self.last_composer_signature
                .as_ref()
                .and_then(|signature| signature.footer_geometry.as_ref()),
        );
        if effect.handled && matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            self.queue_panel.deactivate();
        }
        if let Some(text) = effect.copy {
            use std::io::Write;
            write!(
                self.frame,
                "{}",
                super::fullscreen::selection::osc52_copy(&text)
            )?;
            self.commit_frame()?;
        }
        if effect.handled {
            return Ok(true);
        }
        // 4. 【终端】【沉底点击】左键点击命中待办、队列或子代理面板时立即派发动作并刷新视图
        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            if let Some(action) = self.panel_target_at(mouse.column, mouse.row) {
                self.composer_selection.clear();
                let switches_agent = matches!(action, PanelAction::Agent(_));
                self.apply_panel_action(action);
                if switches_agent {
                    self.replay(self.stream_active)?;
                }
                return Ok(true);
            }
        }
        Ok(hovered)
    }
}
