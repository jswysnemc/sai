//! 【模型选择】【悬停与焦点样式】基于终端面板调色板统一渲染页签、操作按钮、过滤栏、列标题及候选行。

use super::mouse::Target;
use super::render::{truncate, RESET};
use super::state::{PickerColumn, PickerState};
use crate::cli::mouse_menu::ButtonRow;
use crate::i18n::text as t;
use crate::render::panel_palette::{bg, fg, PanelPalette};
use unicode_width::UnicodeWidthStr;

/// 【模型选择】【页签着色】按聚焦列与鼠标悬停状态渲染顶部列切换页签行。
///
/// 参数:
/// - `tabs`: 页签按钮布局
/// - `active`: 当前激活聚焦列
/// - `hovered`: 当前鼠标悬停目标
/// - `palette`: 终端明暗调色板
///
/// 返回:
/// - 保持原始按钮显示列宽的 ANSI 文本
pub(super) fn style_tabs_row(
    tabs: &ButtonRow<Target>,
    active: PickerColumn,
    hovered: Option<Target>,
    palette: PanelPalette,
) -> String {
    let hint_style = fg(palette.muted);
    tabs.render_styled(
        |target| match target {
            Target::Focus(column) if column == active => {
                format!("\x1b[1m{}{}", bg(palette.focus_bg), fg(palette.accent))
            }
            _ if hovered == Some(target) => {
                format!("{}{}", bg(palette.hover_bg), fg(palette.text))
            }
            _ => fg(palette.muted),
        },
        &hint_style,
    )
}

/// 【模型选择】【底栏按钮着色】按主操作强调与鼠标悬停状态渲染底部操作按钮行。
///
/// 参数:
/// - `buttons`: 操作按钮布局
/// - `hovered`: 当前鼠标悬停动作
/// - `primary`: 主操作按钮动作（如保存）
/// - `palette`: 终端明暗调色板
///
/// 返回:
/// - 保持原始按钮显示列宽的 ANSI 文本
pub(super) fn style_action_buttons<T: Copy + Eq>(
    buttons: &ButtonRow<T>,
    hovered: Option<T>,
    primary: Option<T>,
    palette: PanelPalette,
) -> String {
    let hint_style = fg(palette.muted);
    buttons.render_styled(
        |action| {
            if hovered == Some(action) {
                format!("\x1b[1m{}{}", bg(palette.hover_bg), fg(palette.accent))
            } else if primary == Some(action) {
                fg(palette.accent)
            } else {
                fg(palette.text)
            }
        },
        &hint_style,
    )
}

/// 【模型选择】【过滤栏渲染】渲染过滤输入与候选计数，空过滤时为块状光标预留独立单格空白。
///
/// 参数:
/// - `state`: 当前模型选择状态
/// - `width`: 面板可用列宽
/// - `palette`: 终端明暗调色板
///
/// 返回:
/// - 带悬停反馈的过滤栏 ANSI 文本行
pub(super) fn style_filter_line(
    state: &PickerState,
    width: usize,
    palette: PanelPalette,
) -> String {
    // 1. 【模型选择】【悬停判定】鼠标悬停过滤栏时铺设轻量底色并提亮前缀提示
    let prefix = format!("{}: ", t("Filter", "过滤"));
    let suffix = format!("  ({}/{})", state.models().len(), state.total_model_count());
    let hovered = state.hovered() == Some(Target::FilterBar);
    let bg_seq = if hovered {
        bg(palette.hover_bg)
    } else {
        String::new()
    };
    let base_fg = if hovered {
        fg(palette.text)
    } else {
        fg(palette.muted)
    };

    // 2. 【模型选择】【占位留白】过滤词为空时在占位文案前插入单格空格，避免硬件光标遮挡宽字符首字
    if state.filter().is_empty() {
        let full = format!("{prefix} {}{suffix}", t("type to filter", "输入内容过滤"));
        return format!("{bg_seq}{base_fg}{}{RESET}", truncate(&full, width));
    }

    // 3. 【模型选择】【词条高亮】存在过滤输入时保持紧凑前缀并加粗显示用户输入内容
    let budget = width
        .saturating_sub(
            UnicodeWidthStr::width(prefix.as_str()) + UnicodeWidthStr::width(suffix.as_str()),
        )
        .max(1);
    let filter_text = truncate(state.filter(), budget);
    let raw_plain = format!("{prefix}{filter_text}{suffix}");
    if UnicodeWidthStr::width(raw_plain.as_str()) > width {
        format!("{bg_seq}{base_fg}{}{RESET}", truncate(&raw_plain, width))
    } else {
        let input_fg = format!("\x1b[1m{}", fg(palette.text));
        format!(
            "{bg_seq}{base_fg}{prefix}{RESET}{bg_seq}{input_fg}{filter_text}{RESET}{bg_seq}{base_fg}{suffix}{RESET}"
        )
    }
}

/// 【模型选择】【列标题着色】渲染单个列头，非激活列在悬停时铺满当前列宽底色。
///
/// 参数:
/// - `label`: 列头标题文本
/// - `width`: 该列分配宽度
/// - `focused`: 该列是否当前聚焦
/// - `hovered`: 鼠标是否悬停该列标题或页签
/// - `palette`: 终端明暗调色板
///
/// 返回:
/// - 定宽列标题 ANSI 文本
pub(super) fn style_column_title(
    label: &str,
    width: usize,
    focused: bool,
    hovered: bool,
    palette: PanelPalette,
) -> String {
    if width == 0 {
        return String::new();
    }
    // 1. 【模型选择】【标题裁剪】按列宽裁剪指示符与标题并计算右侧补齐空格
    let marker = if focused { "▸ " } else { "  " };
    let text = truncate(&format!("{marker}{label}"), width);
    let padding = width.saturating_sub(UnicodeWidthStr::width(text.as_str()));

    // 2. 【模型选择】【列头底色】聚焦列加粗强调，未聚焦列在悬停时仅铺满自身列宽底色
    if focused {
        format!(
            "\x1b[1m{}{text}{RESET}{}",
            fg(palette.accent),
            " ".repeat(padding)
        )
    } else if hovered {
        format!(
            "{}{}{text}{}{RESET}",
            bg(palette.hover_bg),
            fg(palette.text),
            " ".repeat(padding)
        )
    } else {
        format!("{}{text}{RESET}{}", fg(palette.muted), " ".repeat(padding))
    }
}

/// 【模型选择】【候选单元格着色】独立渲染单列候选项，处理激活焦点条与单列悬停底色。
///
/// 参数:
/// - `label`: 候选项展示文本，空表示该行无此列条目
/// - `selected`: 是否为该列当前选中项
/// - `focused`: 所在列是否当前聚焦
/// - `hovered`: 鼠标是否悬停在该单元格
/// - `width`: 该列分配宽度
/// - `palette`: 终端明暗调色板
///
/// 返回:
/// - 定宽单元格 ANSI 文本，列间留白不染色
pub(super) fn style_item_cell(
    label: Option<&str>,
    selected: bool,
    focused: bool,
    hovered: bool,
    width: usize,
    palette: PanelPalette,
) -> String {
    if width == 0 {
        return String::new();
    }
    let Some(label) = label else {
        return " ".repeat(width);
    };
    // 1. 【模型选择】【前缀标记】仅激活列选中项使用单箭头，保持跨列焦点唯一标识
    let marker = match (selected, focused) {
        (true, true) => "› ",
        (true, false) => "· ",
        _ => "  ",
    };
    let text = truncate(&format!("{marker}{label}"), width);
    let padding = width.saturating_sub(UnicodeWidthStr::width(text.as_str()));

    // 2. 【模型选择】【分态铺底】激活选中铺焦点底色，其余项悬停时仅在本列宽度内铺悬停底色
    if selected && focused {
        format!(
            "\x1b[1m{}{}{text}{}{RESET}",
            bg(palette.focus_bg),
            fg(palette.accent),
            " ".repeat(padding)
        )
    } else if hovered {
        let fg_color = if selected {
            palette.accent
        } else {
            palette.text
        };
        format!(
            "{}{}{text}{}{RESET}",
            bg(palette.hover_bg),
            fg(fg_color),
            " ".repeat(padding)
        )
    } else if selected {
        format!("{}{text}{RESET}{}", fg(palette.accent), " ".repeat(padding))
    } else {
        format!("{}{text}{RESET}{}", fg(palette.muted), " ".repeat(padding))
    }
}

/// 【子任务设置】【列表行样式】计算子任务列表行的选中焦点底色或鼠标悬停底色。
///
/// 参数:
/// - `selected`: 是否为当前选中行
/// - `hovered`: 鼠标是否悬停在该行
/// - `palette`: 终端明暗调色板
///
/// 返回:
/// - 行首 ANSI SGR 样式前缀
pub(super) fn subagent_row_style(
    selected: bool,
    hovered: bool,
    palette: PanelPalette,
) -> String {
    if selected {
        format!("\x1b[1m{}{}", bg(palette.focus_bg), fg(palette.accent))
    } else if hovered {
        format!("{}{}", bg(palette.hover_bg), fg(palette.text))
    } else {
        fg(palette.muted)
    }
}
