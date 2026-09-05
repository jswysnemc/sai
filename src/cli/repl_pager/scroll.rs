use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

/// 滚动条拖动目标。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ScrollDragTarget {
    None,
    /// 底部横向进度条
    Horizontal,
    /// 右侧竖向滚动条
    Vertical,
}

/// 处理鼠标事件（滚轮、点击/拖动进度条）。
///
/// 参数:
/// - `mouse`: 鼠标事件
/// - `cols`: 终端列数
/// - `view_h`: 可视行数
/// - `total_lines`: 正文总行数
/// - `max_scroll`: 最大滚动偏移
/// - `progress_row`: 横向进度条所在行
/// - `body_top_row`: 正文首行所在行
/// - `scroll`: 当前滚动偏移（可写）
/// - `drag_target`: 当前拖动目标（可写）
pub(super) fn apply_mouse(
    mouse: MouseEvent,
    cols: usize,
    view_h: usize,
    total_lines: usize,
    max_scroll: usize,
    progress_row: u16,
    body_top_row: u16,
    scroll: &mut usize,
    drag_target: &mut ScrollDragTarget,
) {
    let body_bottom = body_top_row.saturating_add(view_h as u16);
    let on_horizontal = mouse.row == progress_row
        || mouse.row.saturating_add(1) == progress_row
        || progress_row.saturating_add(1) == mouse.row;
    let on_vertical = mouse.row >= body_top_row
        && mouse.row < body_bottom
        && cols > 0
        && mouse.column as usize + 1 >= cols;
    match mouse.kind {
        MouseEventKind::ScrollUp => {
            *scroll = scroll.saturating_sub(3);
            *drag_target = ScrollDragTarget::None;
        }
        MouseEventKind::ScrollDown => {
            *scroll = (*scroll + 3).min(max_scroll);
            *drag_target = ScrollDragTarget::None;
        }
        MouseEventKind::Down(MouseButton::Left) => {
            if on_horizontal {
                *drag_target = ScrollDragTarget::Horizontal;
                *scroll = scroll_from_track_x(mouse.column as usize, cols, total_lines, view_h);
            } else if on_vertical {
                *drag_target = ScrollDragTarget::Vertical;
                let local = (mouse.row.saturating_sub(body_top_row)) as usize;
                *scroll = scroll_from_vertical_thumb(local, view_h, total_lines, max_scroll);
            } else {
                *drag_target = ScrollDragTarget::None;
            }
        }
        // 部分终端把按住拖动报告为 Moved 而非 Drag
        MouseEventKind::Drag(MouseButton::Left) | MouseEventKind::Moved => {
            if *drag_target == ScrollDragTarget::None {
                return;
            }
            // 拖动中不依赖当前行是否仍在轨道上，避免松脱
            match *drag_target {
                ScrollDragTarget::Horizontal => {
                    *scroll = scroll_from_track_x(mouse.column as usize, cols, total_lines, view_h);
                }
                ScrollDragTarget::Vertical => {
                    let local = if mouse.row < body_top_row {
                        0
                    } else {
                        (mouse.row.saturating_sub(body_top_row) as usize)
                            .min(view_h.saturating_sub(1))
                    };
                    *scroll = scroll_from_vertical_thumb(local, view_h, total_lines, max_scroll);
                }
                ScrollDragTarget::None => {}
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            *drag_target = ScrollDragTarget::None;
        }
        _ => {}
    }
}

/// 由横向进度条点击位置换算 scroll。
///
/// 参数:
/// - `x`: 列位置
/// - `cols`: 总列数
/// - `total_lines`: 总行数
/// - `view_h`: 可视高度
///
/// 返回:
/// - 滚动偏移
fn scroll_from_track_x(x: usize, cols: usize, total_lines: usize, view_h: usize) -> usize {
    let max_scroll = total_lines.saturating_sub(view_h);
    if max_scroll == 0 || cols == 0 {
        return 0;
    }
    let thumb_w = ((cols * view_h) / total_lines.max(1)).clamp(1, cols);
    let travel = cols.saturating_sub(thumb_w).max(1);
    // 点击位置映射到滑块中心所在行程
    let x = x.min(cols.saturating_sub(1));
    let center = x.saturating_sub(thumb_w / 2).min(travel);
    ((center as f64 / travel as f64) * max_scroll as f64).round() as usize
}

/// 由竖向滚动条位置换算 scroll。
///
/// 参数:
/// - `local_row`: 正文区内的相对行
/// - `view_h`: 可视高度
/// - `total_lines`: 总行数
/// - `max_scroll`: 最大滚动
///
/// 返回:
/// - 滚动偏移
fn scroll_from_vertical_thumb(
    local_row: usize,
    view_h: usize,
    total_lines: usize,
    max_scroll: usize,
) -> usize {
    if max_scroll == 0 || view_h == 0 {
        return 0;
    }
    let thumb_h = vertical_thumb_height(view_h, total_lines);
    let travel = view_h.saturating_sub(thumb_h).max(1);
    let center = local_row.saturating_sub(thumb_h / 2).min(travel);
    ((center as f64 / travel as f64) * max_scroll as f64).round() as usize
}

/// 计算竖向滑块高度。
///
/// 参数:
/// - `view_h`: 可视高度
/// - `total_lines`: 总行数
///
/// 返回:
/// - 滑块占用行数
fn vertical_thumb_height(view_h: usize, total_lines: usize) -> usize {
    if total_lines == 0 || total_lines <= view_h {
        return view_h.max(1);
    }
    ((view_h * view_h) / total_lines).clamp(1, view_h)
}

/// 生成右侧竖向滚动条字符。
///
/// 参数:
/// - `view_h`: 可视高度
/// - `total_lines`: 总行数
/// - `scroll`: 当前偏移
///
/// 返回:
/// - 每行一个字符
pub(super) fn scrollbar_glyphs(view_h: usize, total_lines: usize, scroll: usize) -> Vec<char> {
    let mut glyphs = vec!['│'; view_h];
    if view_h == 0 {
        return glyphs;
    }
    if total_lines <= view_h {
        glyphs.fill('█');
        return glyphs;
    }
    let thumb_h = vertical_thumb_height(view_h, total_lines);
    let max_scroll = total_lines.saturating_sub(view_h);
    let travel = view_h.saturating_sub(thumb_h);
    let thumb_top = if max_scroll == 0 {
        0
    } else {
        (scroll * travel) / max_scroll
    };
    for row in thumb_top..thumb_top.saturating_add(thumb_h).min(view_h) {
        glyphs[row] = '█';
    }
    glyphs
}

/// 渲染横向可拖动进度条。
///
/// 参数:
/// - `cols`: 列数
/// - `total_lines`: 总行数
/// - `view_h`: 可视高度
/// - `scroll`: 当前偏移
///
/// 返回:
/// - ANSI 进度条文本
pub(super) fn horizontal_progress_track(
    cols: usize,
    total_lines: usize,
    view_h: usize,
    scroll: usize,
) -> String {
    if cols == 0 {
        return String::new();
    }
    let max_scroll = total_lines.saturating_sub(view_h);
    let mut track = vec!['─'; cols];
    if max_scroll == 0 {
        track.fill('━');
    } else {
        let thumb_w = ((cols * view_h) / total_lines.max(1)).clamp(1, cols);
        let travel = cols.saturating_sub(thumb_w);
        let thumb_x = (scroll * travel) / max_scroll;
        for col in thumb_x..thumb_x.saturating_add(thumb_w).min(cols) {
            track[col] = '━';
        }
    }
    format!("\x1b[36m{}\x1b[0m", track.into_iter().collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_x_maps_edges() {
        assert_eq!(scroll_from_track_x(0, 100, 1000, 10), 0);
        let end = scroll_from_track_x(99, 100, 1000, 10);
        assert!(end >= 900, "end scroll should be near max, got {end}");
    }

    #[test]
    fn vertical_thumb_maps_edges() {
        assert_eq!(scroll_from_vertical_thumb(0, 20, 200, 180), 0);
        let end = scroll_from_vertical_thumb(19, 20, 200, 180);
        assert!(end >= 150, "end scroll should be near max, got {end}");
    }
}
