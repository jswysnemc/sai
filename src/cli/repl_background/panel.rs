//! 嵌入主终端事件循环的后台任务管理状态，不接管终端输入或备用屏幕。

use super::{
    Task, input,
    model::TaskManager,
    service::{Request, TaskService},
    view,
};
use crate::{config::AppConfig, paths::SaiPaths};
use crossterm::event::{Event, KeyEventKind};
use std::time::{Duration, Instant};

#[derive(Default)]
pub(in crate::cli) struct EmbeddedTaskPanel {
    manager: TaskManager,
    service: Option<TaskService>,
    next_refresh: Option<Instant>,
    pub active: bool,
    pub previous: Option<Vec<String>>,
    pub error: Option<String>,
}

impl EmbeddedTaskPanel {
    /// 【后台任务】【环境绑定】更新应用配置，不改变选择、任务快照或进行中的写操作。
    /// 参数: paths 为应用路径，config 为配置；返回: 无。
    pub fn bind(&mut self, paths: &SaiPaths, config: &AppConfig) {
        if self.service.as_ref().is_some_and(TaskService::busy) {
            return;
        }
        self.service = Some(TaskService::new(paths, config));
        self.next_refresh = None;
    }

    /// 【后台任务】【概览快照】提供与管理界面一致的任务范围与排序。
    /// 参数: 无；返回: 非网关任务列表。
    pub fn tasks(&self) -> &[Task] {
        &self.manager.tasks
    }

    /// 【后台任务】【界面关闭】撤下浮层但保留已提交操作，允许权限交互接管屏幕。
    /// 参数: 无；返回: 无。
    pub fn dismiss(&mut self) {
        self.active = false;
        self.previous = None;
        self.next_refresh = None;
        self.manager.confirm = None;
    }

    /// 【后台任务】【界面打开】按实际点击行选择任务，不修改输入草稿或发送控制命令。
    /// 参数: index 为快照中的任务下标，空值表示管理入口；返回: 无。
    pub fn open(&mut self, index: Option<usize>) {
        if let Some(task) = index.and_then(|index| self.manager.tasks.get(index)) {
            self.manager.selected_id = Some(task.id.clone());
        }
        self.manager.filter = Default::default();
        self.manager.pager = false;
        self.manager.confirm = None;
        self.manager.reconcile();
        self.active = true;
        self.previous = None;
        self.next_refresh = None;
        if self.service.is_none() {
            self.manager.loading = false;
        }
    }

    /// 【后台任务】【定时刷新】异步读取概览，管理器打开时额外读取选中任务日志。
    /// 参数: visible 表示侧栏可见；返回: 显示内容是否变化。
    pub fn tick(&mut self, visible: bool) -> bool {
        let Some(service) = self.service.as_mut() else {
            return false;
        };
        let mut changed = false;
        if let Some(result) = service.try_completed() {
            match result {
                Ok(update) => {
                    changed =
                        self.active || self.manager.tasks != update.tasks || self.error.is_some();
                    self.error = None;
                    self.manager.apply(update);
                }
                Err(error) => {
                    let message = error.to_string();
                    changed = self.error.as_ref() != Some(&message);
                    self.error = Some(message.clone());
                    self.manager.notify(message, true);
                }
            }
            self.manager.loading = false;
            self.next_refresh = Some(Instant::now() + Duration::from_secs(2));
        }
        // 1. 【后台任务】【按需读取】隐藏侧栏时不启动读取，写操作和确认期间不发起刷新
        if (visible || self.active)
            && !service.busy()
            && self.manager.confirm.is_none()
            && (self.next_refresh.is_none_or(|next| Instant::now() >= next)
                || (self.active && self.manager.output_id != self.manager.selected_id))
            && tokio::runtime::Handle::try_current().is_ok()
        {
            service.start_with_output(
                Request::Refresh,
                self.manager.selected_id.clone(),
                self.active,
            );
        }
        self.manager.busy = service.mutating();
        changed
    }

    /// 【后台任务】【定时唤醒】空闲输入期间也刷新任务，不依赖对话中已有启动卡片。
    /// 参数: visible 表示侧栏可见；返回: 下次检查间隔，无需读取时为空。
    pub fn wait(&self, visible: bool) -> Option<Duration> {
        let service = self.service.as_ref()?;
        if !visible && !self.active && !service.busy() {
            return None;
        }
        Some(if service.busy() || self.manager.confirm.is_some() {
            Duration::from_millis(100)
        } else {
            self.next_refresh.map_or(Duration::ZERO, |next| {
                next.saturating_duration_since(Instant::now())
            })
        })
    }

    /// 【后台任务】【输入隔离】复用列表、日志、停止确认处理，所有事件都留在浮层内。
    /// 参数: event 为终端事件，size 为当前尺寸；返回: 无。
    pub fn handle(&mut self, event: &Event, size: (u16, u16)) {
        let frame = view::render(&mut self.manager, size);
        let effect = match event {
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                input::handle(&mut self.manager, *key, frame.page_rows)
            }
            Event::Mouse(mouse) => input::mouse(&mut self.manager, *mouse, &frame),
            _ => input::Effect::None,
        };
        match effect {
            input::Effect::Exit if !self.manager.busy => {
                self.dismiss();
            }
            input::Effect::Exit => self.manager.notify(
                crate::i18n::text("Wait for the operation to finish", "请等待当前操作完成").into(),
                false,
            ),
            input::Effect::Request(request) => {
                if let Some(service) = self.service.as_mut() {
                    service.start(request, self.manager.selected_id.clone());
                    self.manager.busy = service.mutating();
                }
            }
            _ => {}
        }
    }

    /// 【后台任务】【浮层绘制】复用自适应管理界面，补齐空行以覆盖旧正文和输入区域。
    /// 参数: size 为终端尺寸；返回: 不超过屏幕范围的完整帧。
    pub fn render(&mut self, size: (u16, u16)) -> Vec<String> {
        let mut lines = view::render(&mut self.manager, size).lines;
        lines.resize(usize::from(size.1), String::new());
        lines
    }
}
