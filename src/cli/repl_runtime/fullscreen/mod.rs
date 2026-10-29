//! Ctrl+O 全屏会话视图：备用屏内整屏接管，正文可滚动，输入框固定在底部。
//!
//! 所有绘制仍经过 `ReplRuntime` 的既有入口（`sync_transcript`、`replay`、
//! `queue_composer`），全屏时这些入口改走本模块的整屏绘制；输入编辑逻辑不变。

mod activity_scroll_anchor;
mod activity_viewport;
mod activity_viewport_hints;
mod activity_visibility;
mod activity_window;
mod bottom_button;
pub(super) mod guide_button;
pub(super) mod hover;
mod hover_color;
mod image_window;
mod input;
mod layout;
mod overview;
mod paint;
#[cfg(test)]
mod presentation_tests;
mod reasoning_viewport;
pub(super) mod selection;
mod state;

pub(in crate::cli) use input::FullscreenEvent;

use super::viewport::{InlineViewport, TerminalSize};
use super::ReplRuntime;
use crate::render::terminal_rows::paint_changed_rows;
use anyhow::Result;
use crossterm::cursor::Hide;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::terminal::{Clear, ClearType};
use crossterm::{execute, queue};
use layout::FullscreenLayout;
use state::FullscreenState;
use std::io::{self, Write};

/// 全屏会话：持有备用屏与鼠标捕获，销毁时恢复主屏。
pub(super) struct FullscreenSession {
    state: FullscreenState,
    /// 输入框正上方“回到底部”按钮的点击范围
    bottom_button: Option<(u16, u16)>,
    /// 待应用的段落切换锚点：段落键与切换前的屏幕偏移
    pending_toggle: Option<(crate::render::transcript::ParagraphKey, isize)>,
    /// 上一帧的图片放置签名
    images: Vec<(usize, String)>,
    /// 松开鼠标后待写入剪贴板的文本
    pending_copy: Option<String>,
}

impl FullscreenSession {
    /// 进入备用屏并开启鼠标捕获（含移动事件，用于悬停预览）。
    ///
    /// 返回:
    /// - 新会话
    fn enter() -> Result<Self> {
        if !cfg!(test) {
            // 备用屏有独立的 Kitty 图片存储，进入时从空记录开始
            crate::render::terminal_image::set_kitty_alternate_screen(true);
            let mut stdout = io::stdout();
            crate::cli::alternate_screen::enter_alternate_screen(&mut stdout)?;
            execute!(stdout, EnableMouseCapture, Clear(ClearType::All), Hide)?;
        }
        Ok(Self {
            state: FullscreenState::new(),
            bottom_button: None,
            pending_toggle: None,
            images: Vec::new(),
            pending_copy: None,
        })
    }
}

impl Drop for FullscreenSession {
    /// 关闭鼠标捕获并回到主屏；错误路径同样执行。
    fn drop(&mut self) {
        if cfg!(test) {
            return;
        }
        let mut stdout = io::stdout();
        let _ = execute!(stdout, DisableMouseCapture);
        let _ = crate::cli::alternate_screen::leave_alternate_screen(&mut stdout);
        let _ = stdout.flush();
        crate::render::terminal_image::set_kitty_alternate_screen(false);
    }
}

impl ReplRuntime {
    /// 【终端】【布局状态】查询当前是否处于全屏，而非读取启动偏好。
    /// 参数: 无；返回: 当前全屏状态。
    pub(in crate::cli) fn fullscreen_active(&self) -> bool {
        self.fullscreen.is_some()
    }

    /// 【全屏视图】【切换】进入或退出全屏会话视图。
    ///
    /// 返回:
    /// - 切换结果
    pub(in crate::cli) fn toggle_fullscreen(&mut self) -> Result<()> {
        if self.fullscreen.is_some() {
            self.leave_fullscreen()
        } else {
            self.enter_fullscreen()
        }
    }

    /// 【全屏视图】【进入】接管备用屏并立即绘制一帧。
    ///
    /// 返回:
    /// - 绘制结果
    pub(in crate::cli) fn enter_fullscreen(&mut self) -> Result<()> {
        if self.fullscreen.is_some() {
            return Ok(());
        }
        // 1. 主屏未提交的绘制先送达，再切到备用屏
        self.commit_frame()?;
        self.fullscreen = Some(FullscreenSession::enter()?);
        // 2. 【终端】【视图同步】呈现方式跟随实际屏幕，避免内联布局进入备用屏
        crate::render::tui_mode::set_override(Some(crate::config::TuiMode::Fullscreen));
        self.resize_preview = None;
        self.last_composer_signature = None;
        self.paint_fullscreen()
    }

    /// 【全屏视图】【退出】回到主屏并从 source 全量重放。
    ///
    /// 全屏期间主屏没有被绘制，增量记账已失效，必须整区重锚。
    ///
    /// 返回:
    /// - 重放结果
    pub(in crate::cli) fn leave_fullscreen(&mut self) -> Result<()> {
        let Some(session) = self.fullscreen.take() else {
            return Ok(());
        };
        drop(session);
        // 1. 【终端】【视图同步】回到主屏后按内联视图重放
        crate::render::tui_mode::set_override(Some(crate::config::TuiMode::Inline));
        self.last_composer_signature = None;
        self.force_reanchor = true;
        self.replay(self.stream_active)
    }

    /// 【全屏视图】【绘制】重排正文并整屏差异绘制，输入框随后画在固定底部。
    ///
    /// 返回:
    /// - 绘制结果
    pub(super) fn paint_fullscreen(&mut self) -> Result<()> {
        crate::render::tui_presentation::with_locale(|| self.paint_fullscreen_view(true))
    }

    /// 【全屏视图】【交互重绘】滚动和拖选复用正文，仅在尺寸或展开状态变化时重新排版。
    ///
    /// 参数: 无
    /// 返回: 可见区域绘制结果
    pub(super) fn paint_fullscreen_interaction(&mut self) -> Result<()> {
        crate::render::tui_presentation::with_locale(|| self.paint_fullscreen_view(false))
    }

    /// 【全屏视图】【分区绘制】按来源决定是否更新正文，再绘制可见行与固定输入区。
    ///
    /// 参数:
    /// - `refresh_document`: 会话内容或流式状态变化时为真，纯交互重绘时为假
    ///
    /// 返回:
    /// - 绘制结果
    fn paint_fullscreen_view(&mut self, refresh_document: bool) -> Result<()> {
        if self.fullscreen.is_none() {
            return Ok(());
        }
        let size = TerminalSize::current();
        // 1. 沉底面板可能随实时状态变化，先刷新再计算输入框高度；
        //    面板里的折叠提示同样不能引导用户按 Ctrl+O
        let panel = crate::render::omitted_line::with_fullscreen_hints(|| {
            self.bottom_panel_lines(usize::from(size.cols))
        });
        if let Some(composer) = self.composer.as_mut() {
            composer.set_panel_lines(panel);
        }
        let wanted = self.composer_height_for(size);
        let layout = FullscreenLayout::compute(size.cols, size.rows, wanted);
        let Some(session) = self.fullscreen.as_mut() else {
            return Ok(());
        };
        // 2. 【全屏视图】【正文复用】拖选和滚动不遍历历史；内容、布局或展开状态变化才更新正文
        let layout_changed = session.state.layout != Some(layout);
        if layout_changed {
            session.state.previous = None;
        }
        let body_height = usize::from(layout.body_height);
        if refresh_document || layout_changed || session.pending_toggle.is_some() {
            let document = self.transcript.render_fullscreen(
                usize::from(layout.content_width),
                &self.options,
                &session.state.expanded,
            );
            self.transcript.clear_dirty();
            let document = session
                .state
                .reasoning
                .update(document, usize::from(layout.content_width));
            let document = session.state.activity.update(
                document,
                body_height,
                usize::from(layout.content_width),
                usize::from(size.rows),
            );
            session.state.apply_document(document, body_height);
        }
        if let Some((key, offset)) = session.pending_toggle.take() {
            session
                .state
                .restore_toggle_anchor(key, offset, body_height);
        }
        // 3. 组装标题与正文，只重写变化的行
        let painted = paint::compose(&session.state, &layout);
        // 图片位置变化时先删除旧放置并整屏重画，未变化的行也要重新落下图片
        let images = image_window::placement_signature(&painted.rows);
        if session.images != images {
            session.state.previous = None;
            session.images = images;
        }
        if session.state.previous.is_none() {
            write!(
                self.frame,
                "{}",
                crate::render::terminal_image::kitty_delete_placements()
            )?;
        }
        queue!(self.frame, Hide)?;
        paint_changed_rows(
            &mut self.frame,
            0,
            usize::from(layout.cols),
            &painted.rows,
            session.state.previous.as_deref(),
        )?;
        session.state.previous = Some(painted.rows);
        session.state.layout = Some(layout);
        session.bottom_button = painted.bottom_button;
        // 4. 输入框画在固定底部；没有输入框时光标保持隐藏
        self.viewport = InlineViewport::fixed(size, layout.composer_top, layout.composer_height);
        self.queue_composer()?;
        self.commit_frame()
    }

    /// 丢弃差异基线后整屏重画（Ctrl+L、尺寸变化、清屏）。
    ///
    /// 返回:
    /// - 绘制结果
    pub(super) fn repaint_fullscreen(&mut self) -> Result<()> {
        if let Some(session) = self.fullscreen.as_mut() {
            session.state.previous = None;
        }
        self.last_composer_signature = None;
        if !cfg!(test) {
            queue!(self.frame, Clear(ClearType::All))?;
        }
        self.paint_fullscreen()
    }

    /// 全屏时输入框变化后只在分区改变时整屏重排，否则仅刷新输入框。
    ///
    /// 参数:
    /// - `size`: 当前终端尺寸
    ///
    /// 返回:
    /// - 输入框顶部行与高度
    pub(super) fn update_fullscreen_composer(&mut self, size: TerminalSize) -> Result<(u16, u16)> {
        let panel = crate::render::omitted_line::with_fullscreen_hints(|| {
            self.bottom_panel_lines(usize::from(size.cols))
        });
        if let Some(composer) = self.composer.as_mut() {
            composer.set_panel_lines(panel);
        }
        let wanted = self.composer_height_for(size);
        let layout = FullscreenLayout::compute(size.cols, size.rows, wanted);
        let stale = self
            .fullscreen
            .as_ref()
            .is_some_and(|session| session.state.layout != Some(layout));
        if stale {
            self.paint_fullscreen()?;
        } else {
            self.viewport =
                InlineViewport::fixed(size, layout.composer_top, layout.composer_height);
        }
        Ok((layout.composer_top, layout.composer_height))
    }

    /// 权限等待时回到底部，保证审批控件可见。
    ///
    /// 返回:
    /// - 无
    pub(super) fn follow_fullscreen_bottom(&mut self) {
        if let Some(session) = self.fullscreen.as_mut() {
            session.state.follow = true;
        }
    }
}

#[cfg(test)]
#[path = "hover_tests.rs"]
mod hover_tests;
#[cfg(test)]
#[path = "runtime_tests.rs"]
mod runtime_tests;

#[cfg(test)]
mod copy_tests;
#[cfg(test)]
mod todo_tests;

#[cfg(test)]
mod activity_tests;
