//! 顶部紧凑位置指示和全终端右端的侧栏开关。

use super::input::Effect;
use super::layout::FullscreenLayout;
use super::sidebar_control::{SidebarControl, Visibility};
use super::state::FullscreenState;
use crate::cli::repl_runtime::ReplRuntime;
use crate::render::panel_palette::{bg, fg, PanelPalette, Rgb};
use crate::render::terminal_palette::terminal_palette;
use crossterm::event::{Event, MouseButton, MouseEventKind};
use std::ops::Range;

/// Nerd Fonts 的 Font Awesome columns 字形，沿用项目现有单列图标体系。
pub(super) const SIDEBAR_ICON: &str = "\u{f0db}";
/// 禁用 Nerd Fonts 时回退的标准单列分栏几何字符。
pub(super) const FALLBACK_SIDEBAR_ICON: &str = "◫";
const CONTROL_WIDTH: usize = 3;

/// 【终端标题】【图标选择】按环境变量开关返回侧栏图标，禁用 Nerd Font 时回退标准几何字符。
/// 参数: `nerd_enabled` 为是否启用 Nerd Font；返回: 单列宽图标字符串。
pub(super) fn sidebar_icon_for(nerd_enabled: bool) -> &'static str {
    if nerd_enabled {
        SIDEBAR_ICON
    } else {
        FALLBACK_SIDEBAR_ICON
    }
}

/// 【终端标题】【当前图标】读取全局 Nerd Font 开关返回当前生效的侧栏图标。
/// 参数: 无；返回: 单列宽图标字符串。
#[cfg(test)]
pub(super) fn current_sidebar_icon() -> &'static str {
    sidebar_icon_for(crate::render::nerd_font::nerd_font_enabled())
}

/// 【终端标题】【开关范围】侧栏开关固定在终端最右端，仅在可手动开启时显示。
/// 参数: layout 为当前布局；返回: 开关列范围，不含右端留白。
pub(super) fn target(layout: &FullscreenLayout) -> Option<Range<usize>> {
    SidebarControl::can_open(layout.cols, layout.rows).then(|| {
        let end = usize::from(layout.cols).saturating_sub(1);
        end.saturating_sub(CONTROL_WIDTH)..end
    })
}

/// 【终端标题】【标题预留】无侧栏时为开关预留四列，有侧栏时标题使用完整聊天宽度。
/// 参数: layout 为当前布局；返回: 左侧标题可用列数。
pub(super) fn title_width(layout: &FullscreenLayout) -> usize {
    let reserve =
        usize::from(layout.sidebar.is_none() && target(layout).is_some()) * (CONTROL_WIDTH + 1);
    usize::from(layout.main_width).saturating_sub(reserve)
}

/// 【终端标题】【位置指示】用短轨道和百分比显示正文位置，接入 PanelPalette 明暗适配色。
/// 参数: state 为正文状态，layout 为布局；返回: 紧凑 ANSI 位置文本。
pub(super) fn position(state: &FullscreenState, layout: &FullscreenLayout) -> String {
    if state.document.lines.is_empty() {
        return String::new();
    }
    let palette = PanelPalette::current();
    let max = state.max_scroll(usize::from(layout.body_height));
    let percent = if max == 0 {
        100
    } else {
        state.scroll.min(max) * 100 / max
    };
    let color = fg(if state.unseen {
        palette.accent
    } else {
        palette.muted
    });
    if title_width(layout) < 70 {
        return format!("{color}{percent}%");
    }
    let marker = percent * 4 / 100;
    let track = (0..5)
        .map(|index| if index == marker { '●' } else { '─' })
        .collect::<String>();
    format!("{color}{track} {percent:>3}%")
}

/// 【终端标题】【开关绘制】按当前终端背景与字体开关替换顶行最右端，默认正文色，悬停使用主题色。
/// 参数: control 为开关状态，layout 为布局，rows 为整屏行；返回: 无。
pub(super) fn overlay(control: &SidebarControl, layout: &FullscreenLayout, rows: &mut [String]) {
    overlay_for_background(
        control,
        layout,
        rows,
        terminal_palette().background,
        crate::render::nerd_font::nerd_font_enabled(),
    );
}

/// 【终端标题】【主题开关绘制】使用指定背景色与图标开关绘制右上角侧栏按钮，浅色背景不落黑底。
/// 参数: `control` 为开关状态，`layout` 为布局，`rows` 为整屏行，`background` 为终端背景，`nerd_enabled` 为图标开关；返回: 无。
pub(super) fn overlay_for_background(
    control: &SidebarControl,
    layout: &FullscreenLayout,
    rows: &mut [String],
    background: Rgb,
    nerd_enabled: bool,
) {
    let (Some(range), Some(row)) = (target(layout), rows.first_mut()) else {
        return;
    };
    let palette = PanelPalette::for_background(background);
    let active = layout.sidebar.is_some();
    let bar_bg = if active {
        "\x1b[49m".to_string()
    } else if super::paint::is_light_bg(background) {
        bg(palette.hover_bg)
    } else {
        "\x1b[48;5;235m".to_string()
    };
    let color = fg(if control.header_hovered {
        palette.accent
    } else {
        palette.text
    });
    let text = format!(" {} ", sidebar_icon_for(nerd_enabled));
    let left = crate::render::ansi_clip::clip_ansi_to_width(row, range.start, "");
    let pad = range
        .start
        .saturating_sub(crate::cli::repl_text::visible_width(&left));
    *row = format!("{left}{bar_bg}{}{color}{text} \x1b[0m", " ".repeat(pad));
}

impl ReplRuntime {
    /// 【终端标题】【侧栏交互】顶部开关立即切换侧栏，保留草稿、宽度偏好和正文滚动状态。
    /// 参数: event 为终端事件；返回: 处理效果，其他事件为空。
    pub(super) fn header_control_event(&mut self, event: &Event) -> Option<Effect> {
        let session = self.fullscreen.as_mut()?;
        let layout = session.state.layout?;
        let Event::Mouse(mouse) = event else {
            if matches!(event, Event::Resize(..) | Event::FocusLost) {
                self.sidebar_control.header_hovered = false;
            }
            return None;
        };
        let hit = mouse.row == 0
            && target(&layout).is_some_and(|range| range.contains(&usize::from(mouse.column)));
        if mouse.kind == MouseEventKind::Moved {
            let changed = self.sidebar_control.header_hovered != hit;
            self.sidebar_control.header_hovered = hit;
            if hit {
                self.sidebar_control.divider_hovered = false;
                session.state.header_hover = false;
                session.state.pointer_row = None;
                session.state.pointer_col = None;
                session.sidebar.hovered = None;
                session.sidebar.preview.hovered = None;
                return Some(if changed {
                    Effect::Repaint
                } else {
                    Effect::Nothing
                });
            }
            return None;
        }
        if !hit {
            return None;
        }
        if matches!(mouse.kind, MouseEventKind::Up(MouseButton::Left))
            && (session.sidebar.preview.selection.is_pressed()
                || session.state.press.is_some()
                || session.state.dragging)
        {
            return None;
        }
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if layout.sidebar.is_some() {
                    self.sidebar_control.close();
                    session.sidebar.preview.close();
                } else {
                    self.sidebar_control.visibility = Some(Visibility::On);
                }
                session.sidebar.focused = false;
                session.sidebar.hovered = None;
                session.state.press = None;
                session.state.selection = None;
                session.state.dragging = false;
                session.state.header_hover = false;
                self.composer_selection.clear();
                Some(Effect::Refresh)
            }
            MouseEventKind::Up(MouseButton::Left) => Some(Effect::Nothing),
            _ => None,
        }
    }
}
