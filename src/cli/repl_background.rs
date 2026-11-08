//! 后台任务管理入口，列表和日志视图共享状态与异步任务服务。

mod input;
mod list_body;
mod model;
#[cfg(test)]
mod model_tests;
mod pager_view;
mod panel;
mod panel_rect;
mod screen;
mod service;
mod task_style;
mod view;
mod view_hints;
mod view_list;
#[cfg(test)]
mod view_tests;

use crate::config::AppConfig;
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::Result;
use crossterm::event::{self, Event, KeyEventKind};
use model::TaskManager;
use service::{Request, TaskService};
use std::time::{Duration, Instant};

pub(in crate::cli) use model::Task;
pub(in crate::cli) use panel::EmbeddedTaskPanel;
pub(in crate::cli) use panel_rect::PanelRect;
pub(in crate::cli) use task_style::status_style;

/// 【后台任务】【管理入口】运行占满终端的任务面板，接口请求不阻塞键盘与终端尺寸更新。
/// 参数: paths 为应用路径，config 为当前配置；返回: 终端交互结果。
pub(super) async fn run_repl_background_manager(
    paths: &SaiPaths,
    config: &AppConfig,
) -> Result<()> {
    let mut screen = screen::TaskScreen::start()?;
    let mut manager = TaskManager::default();
    let mut service = TaskService::new(paths, config);
    let mut next_refresh = Instant::now();
    loop {
        // 1. 【后台任务】【异步同步】读取完成的结果，错误留在面板中，不结束 REPL
        if let Some(result) = service.completed().await {
            match result {
                Ok(update) => manager.apply(update),
                Err(error) => manager.notify(error.to_string(), true),
            }
            manager.loading = false;
            next_refresh = Instant::now() + Duration::from_secs(2);
        }
        if !service.busy() && Instant::now() >= next_refresh && manager.confirm.is_none() {
            service.start(Request::Refresh, manager.selected_id.clone());
        }
        manager.busy = service.mutating();
        let size = crossterm::terminal::size()?;
        let frame = view::render(&mut manager, PanelRect::inset(size.0, 0, size.1));
        screen.draw(&frame.placed(), size)?;

        // 2. 【后台任务】【输入分流】日志模式与列表模式独立处理，释放事件不重复执行操作
        if !event::poll(Duration::from_millis(80))? {
            continue;
        }
        let effect = match event::read()? {
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                input::handle(&mut manager, key, frame.page_rows)
            }
            Event::Mouse(mouse) => input::mouse(&mut manager, mouse, &frame),
            _ => input::Effect::None,
        };
        match effect {
            input::Effect::Exit if !service.mutating() => break,
            input::Effect::Exit => manager.notify(
                t("Wait for the operation to finish", "请等待当前操作完成").into(),
                false,
            ),
            input::Effect::Request(request) if !service.mutating() => {
                service.start(request, manager.selected_id.clone());
            }
            _ => {}
        }
        if manager.output_id != manager.selected_id && !service.busy() {
            next_refresh = Instant::now();
        }
    }
    Ok(())
}
