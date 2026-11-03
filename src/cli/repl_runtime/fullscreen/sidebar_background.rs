//! 后台任务面板与主终端的非阻塞交接，绘制和输入始终由原事件循环驱动。

use crate::cli::repl_runtime::{ReplRuntime, viewport::TerminalSize};
use crate::{config::AppConfig, paths::SaiPaths};
use anyhow::Result;
use crossterm::{cursor::Hide, event::Event, queue};
use std::io::Write;

impl ReplRuntime {
    /// 【终端侧栏】【概览刷新】分别采样任务和 Git，任一分组变化即请求重绘。
    /// 参数: 无；返回: 是否有可见状态变化。
    pub(in crate::cli::repl_runtime) fn tick_sidebar(&mut self) -> bool {
        let background = self.tick_sidebar_background();
        self.tick_sidebar_git() || background
    }

    /// 【后台任务】【运行环境】绑定用户级管理接口，不限制到当前会话的启动卡片。
    /// 参数: paths 为路径，config 为配置；返回: 无。
    pub(in crate::cli) fn configure_background_tasks(
        &mut self,
        paths: &SaiPaths,
        config: &AppConfig,
    ) {
        self.background_panel.bind(paths, config);
    }

    /// 【后台任务】【浮层状态】判断输入和自动轮次是否需要让位给管理界面。
    /// 参数: 无；返回: 管理界面是否打开。
    pub(in crate::cli) fn background_panel_active(&self) -> bool {
        self.fullscreen.is_some() && self.background_panel.active
    }

    /// 【后台任务】【概览刷新】在空闲和流式阶段共用侧栏刷新周期。
    /// 参数: 无；返回: 任务快照是否变化。
    pub(super) fn tick_sidebar_background(&mut self) -> bool {
        self.background_panel
            .tick(self.sidebar_visible(usize::from(TerminalSize::current().cols)))
    }

    /// 【后台任务】【浮层输入】拦截管理器事件，关闭后恢复原会话和输入框，不发送聊天消息。
    /// 参数: event 为终端事件；返回: 是否消费事件。
    pub(super) fn background_panel_event(&mut self, event: &Event) -> Result<bool> {
        if !self.background_panel_active() {
            return Ok(false);
        }
        let size = TerminalSize::current();
        self.background_panel.handle(event, (size.cols, size.rows));
        self.paint_fullscreen()?;
        Ok(true)
    }

    /// 【后台任务】【浮层输出】覆盖完整终端帧并撤销旧绘制基线，后续关闭时完整恢复。
    /// 参数: 无；返回: 帧写入结果，由调用方提交。
    pub(in crate::cli::repl_runtime) fn queue_background_panel(&mut self) -> Result<()> {
        let size = TerminalSize::current();
        let lines = self.background_panel.render((size.cols, size.rows));
        queue!(self.frame, Hide)?;
        if self.background_panel.previous.is_none() {
            write!(
                self.frame,
                "{}",
                crate::render::terminal_image::kitty_delete_placements()
            )?;
        }
        crate::render::terminal_rows::paint_changed_rows(
            &mut self.frame,
            0,
            usize::from(size.cols),
            &lines,
            self.background_panel.previous.as_deref(),
        )?;
        self.background_panel.previous = Some(lines);
        self.last_composer_signature = None;
        if let Some(session) = self.fullscreen.as_mut() {
            session.state.previous = None;
        }
        Ok(())
    }
}
