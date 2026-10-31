//! 侧栏会话偏好与固定分栏尺寸；不依赖绘制缓存或终端鼠标事件。

use super::layout::{FullscreenLayout, SidebarLayout};
use crate::cli::repl_runtime::ReplRuntime;
use crate::config::SidebarDefault;
use anyhow::Result;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Visibility {
    Auto,
    On,
    Off,
}

impl Visibility {
    /// 【终端侧栏】【状态名称】返回命令使用的稳定状态名称。
    /// 参数: 当前状态；返回: auto、on 或 off。
    fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::On => "on",
            Self::Off => "off",
        }
    }
}

#[derive(Default)]
pub(in crate::cli::repl_runtime) struct SidebarControl {
    pub(super) default: SidebarDefault,
    pub(super) visibility: Option<Visibility>,
    pub(super) width: Option<u16>,
    pub(super) review_width: Option<u16>,
    pub(super) dragging: bool,
    pub(super) divider_hovered: bool,
    pub(super) close_hovered: bool,
    pub(super) header_hovered: bool,
}

impl SidebarControl {
    /// 【终端侧栏】【开启条件】统一手动开启的尺寸下限，供命令策略和顶部开关共用。
    /// 参数: cols/rows 为终端尺寸；返回: 是否足以保留聊天和侧栏。
    pub(super) fn can_open(cols: u16, rows: u16) -> bool {
        cols >= 88 && rows >= 8
    }

    /// 【终端侧栏】【有效状态】显式选择优先于配置，配置刷新不覆盖手动开关。
    /// 参数: 当前状态；返回: 生效的显示策略。
    pub(super) fn visibility(&self) -> Visibility {
        self.visibility.unwrap_or(match self.default {
            SidebarDefault::Auto => Visibility::Auto,
            SidebarDefault::Off => Visibility::Off,
        })
    }

    /// 【终端侧栏】【分栏尺寸】限制用户宽度，分别保留普通侧栏和审查区的调整结果。
    /// 参数: cols/rows 为终端尺寸，review 为审查状态；返回: 固定侧栏坐标或隐藏。
    pub(super) fn geometry(&self, cols: u16, rows: u16, review: bool) -> Option<SidebarLayout> {
        let cols = cols.max(8);
        if review {
            let minimum = 32.min(cols / 2);
            let maximum = cols.saturating_sub(40.min(cols / 2)).max(minimum);
            let width = self
                .review_width
                .unwrap_or(cols - cols / 2)
                .clamp(minimum, maximum);
            return Some(SidebarLayout {
                left: cols - width,
                width,
            });
        }
        let automatic = FullscreenLayout::sidebar_for(cols, rows);
        match self.visibility() {
            Visibility::Off => return None,
            Visibility::Auto if automatic.is_none() => return None,
            Visibility::On if !Self::can_open(cols, rows) => return None,
            _ => {}
        }
        let width = self
            .width
            .unwrap_or((cols / 4).clamp(36, 44))
            .clamp(28, cols - 60);
        Some(SidebarLayout {
            left: cols - width,
            width,
        })
    }

    /// 【终端侧栏】【统一布局】正文、输入框和面板共用尺寸策略，避免命中位置偏离绘制。
    /// 参数: cols/rows 为终端尺寸，wanted 为输入高度，review 为审查状态；返回: 屏幕分区。
    pub(super) fn layout(
        &self,
        cols: u16,
        rows: u16,
        wanted: u16,
        review: bool,
    ) -> FullscreenLayout {
        FullscreenLayout::with_sidebar(
            cols,
            rows,
            wanted,
            review,
            self.geometry(cols, rows, review),
        )
    }

    /// 【终端侧栏】【关闭面板】手动隐藏并清除鼠标状态，终端放大后也不会自动重开。
    /// 参数: 当前控制状态；返回: 无。
    pub(super) fn close(&mut self) {
        self.visibility = Some(Visibility::Off);
        self.dragging = false;
        self.divider_hovered = false;
        self.close_hovered = false;
    }
}

impl ReplRuntime {
    /// 【终端侧栏】【默认配置】更新启动偏好，保留本会话的手动开关和宽度。
    /// 参数: default 为配置默认值；返回: 无。
    pub(in crate::cli) fn configure_sidebar(&mut self, default: SidebarDefault) {
        self.sidebar_control.default = default;
    }

    /// 【终端侧栏】【开关命令】处理空闲及生成期间的同一命令，不修改模型和消息队列。
    /// 参数: input 为命令原文；返回: 是否命中 /tui，绘制错误向调用者返回。
    pub(in crate::cli) fn handle_tui_command(&mut self, input: &str) -> Result<bool> {
        let Some(rest) = crate::cli::repl_commands::repl_command_rest(input, "/tui") else {
            return Ok(false);
        };
        let args = rest
            .split_whitespace()
            .map(str::to_ascii_lowercase)
            .collect::<Vec<_>>();
        let visibility = match args
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .as_slice()
        {
            ["sidebar", "on"] => Some(Visibility::On),
            ["sidebar", "off"] => Some(Visibility::Off),
            ["sidebar", "auto"] => Some(Visibility::Auto),
            ["sidebar"] => None,
            _ => {
                self.record_meta(
                    crate::i18n::text(
                        "usage: /tui sidebar [on|off|auto]",
                        "用法: /tui sidebar [on|off|auto]",
                    )
                    .to_string(),
                )?;
                return Ok(true);
            }
        };
        // 1. 【终端侧栏】【手动选择】关闭时同时退出审查，开启时确保进入全屏视图
        if let Some(visibility) = visibility {
            self.sidebar_control.visibility = Some(visibility);
            self.sidebar_control.dragging = false;
            self.sidebar_control.close_hovered = false;
            self.sidebar_control.divider_hovered = false;
            if let Some(session) = self.fullscreen.as_mut() {
                session.sidebar.focused = false;
                session.sidebar.hovered = None;
                if visibility == Visibility::Off {
                    session.sidebar.preview.close();
                }
            }
            if visibility != Visibility::Off {
                self.enter_fullscreen()?;
            }
            self.redraw()?;
        }
        self.record_meta(format!(
            "sidebar: {}",
            self.sidebar_control.visibility().name()
        ))?;
        Ok(true)
    }
}
