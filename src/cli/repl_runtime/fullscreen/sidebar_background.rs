//! 后台任务面板与主终端的非阻塞交接，绘制和输入始终由原事件循环驱动。

use crate::cli::repl_runtime::{ReplRuntime, viewport::TerminalSize};
use crate::{config::AppConfig, paths::SaiPaths};
use anyhow::Result;
use crossterm::event::Event;

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
        // 1. 【后台任务】【命中几何】使用上一帧实际绘制的分区，保证鼠标坐标与可见面板一致
        let rect = self
            .fullscreen
            .as_ref()
            .and_then(|session| session.state.layout)
            .map_or_else(
                || {
                    let size = TerminalSize::current();
                    crate::cli::repl_background::PanelRect::inset(size.cols, 0, size.rows)
                },
                |layout| super::background_overlay::panel_rect(&layout),
            );
        self.background_panel.handle(event, rect);
        self.paint_fullscreen()?;
        Ok(true)
    }
}
