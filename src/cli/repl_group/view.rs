//! 群组工作台整屏两栏布局编排：完全对齐 Sai 全屏视图结构（左侧主画布 + 底部原生输入区，
//! 右侧全高可折叠统一侧栏，窄屏自动收起侧栏保持主画布与输入区完整可用）。

use super::chat_view::render_chat_pane;
use super::composer::{measure_composer_height, render_native_composer};
use super::header_view::{overlay_sidebar_toggle, render_main_header};
use super::modal_view::overlay_modal;
use super::sidebar_view::render_right_sidebar;
use super::state::GroupTuiState;
use super::text_util::pad_ansi;
use crate::render::panel_palette::PanelPalette;

/// 允许展开右侧统一侧栏所需的最小终端列数。
pub(super) const SIDEBAR_MIN_COLS: usize = 100;

/// 整屏渲染输出（包含屏幕行缓冲与真实终端光标位置）。
pub(super) struct RenderedFrame {
    pub(super) lines: Vec<String>,
    pub(super) cursor: Option<(u16, u16)>,
}

/// 【群组终端】【整屏布局】按 Sai 全屏两栏结构编排顶栏、群聊正文、原生输入区与右侧统一侧栏。
///
/// 参数:
/// - `state`: 群组 TUI 状态（同步写入所有可点击区域热区）
/// - `size`: 终端 `(cols, rows)`
///
/// 返回:
/// - 长度严格等于 `rows` 的整屏渲染帧及真实硬件光标坐标
pub(super) fn render(state: &mut GroupTuiState, size: (u16, u16)) -> RenderedFrame {
    let cols = usize::from(size.0).max(48);
    let rows = usize::from(size.1).max(14);
    let palette = PanelPalette::current();
    state.hit_map = Default::default();

    // 1. 计算左右两栏列宽（对齐 `fullscreen/layout.rs`：左栏统筹正文与输入区，右栏全高独立）
    let can_open_sidebar = cols >= SIDEBAR_MIN_COLS;
    let show_sidebar = state.inspector_open && can_open_sidebar;
    let sidebar_w = if show_sidebar {
        (cols * 31 / 100).clamp(34, 44)
    } else {
        0usize
    };
    let main_w = cols.saturating_sub(sidebar_w).max(28);
    let chat_w = if show_sidebar {
        main_w.saturating_sub(1).max(24)
    } else {
        main_w
    };

    // 2. 计算左栏纵向分区：第 0 行顶栏，底部自适应高度原生输入区，中间为群聊正文
    let header_h = 1usize;
    let min_chat_rows = 4usize.min(rows.saturating_sub(header_h + 5).max(1));
    let composer_max_h = rows.saturating_sub(header_h + min_chat_rows).max(5);
    let composer_h = measure_composer_height(state, chat_w, composer_max_h);
    let body_h = rows.saturating_sub(header_h + composer_h).max(1);
    let composer_top = header_h + body_h;

    state.hit_map.sidebar_start_col = main_w as u16;
    state.hit_map.chat_col_range = (0, main_w as u16);
    state.hit_map.chat_row_range = (header_h as u16, composer_top as u16);
    state.hit_map.composer_row_range = (composer_top as u16, rows as u16);

    // 3. 分别渲染左侧正文、左侧原生输入区与右侧统一侧栏（先绘正文以更新总行数供顶栏显示进度）
    let chat_lines = render_chat_pane(state, chat_w, body_h, header_h, &palette);
    let header_line =
        render_main_header(state, main_w, !show_sidebar && can_open_sidebar, &palette);
    let composer = render_native_composer(state, chat_w, composer_h, composer_top, &palette);
    let sidebar_lines = if show_sidebar {
        render_right_sidebar(state, sidebar_w, rows, main_w, &palette)
    } else {
        Vec::new()
    };

    // 4. 拼装左侧主栏（顶栏 + 正文 + 原生输入区）与右侧全高侧栏
    let mut left_column = Vec::with_capacity(rows);
    left_column.push(header_line);
    for line in chat_lines {
        if show_sidebar {
            left_column.push(format!("{} ", pad_ansi(&line, chat_w)));
        } else {
            left_column.push(pad_ansi(&line, main_w));
        }
    }
    for line in composer.lines {
        if show_sidebar {
            left_column.push(format!("{} ", pad_ansi(&line, chat_w)));
        } else {
            left_column.push(pad_ansi(&line, main_w));
        }
    }
    while left_column.len() < rows {
        left_column.push(" ".repeat(main_w));
    }
    left_column.truncate(rows);

    let mut out = Vec::with_capacity(rows);
    for row in 0..rows {
        let left = pad_ansi(&left_column[row], main_w);
        if show_sidebar {
            let right = pad_ansi(
                sidebar_lines.get(row).map(String::as_str).unwrap_or(""),
                sidebar_w,
            );
            out.push(format!("{left}{right}"));
        } else {
            out.push(left);
        }
    }

    // 5. 在第 0 行最右端叠加可点击的侧栏切换图标 `◫`
    if let Some(first_row) = out.first_mut() {
        state.hit_map.header_sidebar_toggle = overlay_sidebar_toggle(
            first_row,
            cols,
            show_sidebar,
            can_open_sidebar,
            state.pointer,
            &palette,
        );
    }

    let mut cursor = Some((
        composer.cursor.0.min(chat_w.saturating_sub(1) as u16),
        (composer_top as u16)
            .saturating_add(composer.cursor.1)
            .min(rows.saturating_sub(1) as u16),
    ));

    // 6. 叠加居中圆角弹窗（主画布足够宽时局限在左栏中央，保持右侧栏完整不被截断）
    if let Some(modal) = &state.modal {
        let modal_cols = if show_sidebar && chat_w >= 62 {
            chat_w
        } else {
            cols
        };
        cursor = overlay_modal(
            &mut out,
            modal,
            &state.gateway_status,
            &state.model_info,
            modal_cols,
            rows,
            &palette,
        );
    }

    RenderedFrame { lines: out, cursor }
}
