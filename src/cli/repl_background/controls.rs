//! 点击区域从实际绘制的文本计算，危险操作仍经过原有固定目标确认。

use super::{model::TaskManager, view::TaskFrame};
use crossterm::event::{KeyCode, MouseEvent};

/// 【后台任务】【控件命中】参数为状态、鼠标与已渲染帧，返回对应原有键盘动作
pub(super) fn hit(manager: &TaskManager, mouse: MouseEvent, frame: &TaskFrame) -> Option<KeyCode> {
    let local_row = usize::from(mouse.row.checked_sub(frame.rect.y)?);
    let column = usize::from(mouse.column.checked_sub(frame.rect.x)?);
    let line = crate::render::ansi_clip::strip_ansi(frame.lines.get(local_row)?);
    if local_row + 1 == usize::from(frame.rect.height) {
        // 1. 【后台任务】【页脚命中】只接受实际显示的完整分段，通知与繁忙文案没有操作区域
        let mut start = 2;
        for segment in line.trim_start_matches(['╰', ' ']).split(" · ") {
            let segment = segment.trim_end_matches(['─', '╯', ' ']);
            let width = crate::render::ansi_clip::ansi_visible_width(segment);
            if column >= start && column < start + width {
                let key = segment.split_whitespace().next()?;
                return match key {
                    "Enter/y" | "Enter" => Some(KeyCode::Enter),
                    "Esc/n" | "Esc" => Some(KeyCode::Esc),
                    "Tab" => Some(KeyCode::Tab),
                    "s" | "f" | "x" | "r" => Some(KeyCode::Char(key.chars().next()?)),
                    "g/G" => Some(KeyCode::Home),
                    "PgUp/PgDn" => Some(KeyCode::PageDown),
                    "↑↓" => Some(KeyCode::Down),
                    _ => None,
                };
            }
            start += width + 3;
        }
    } else if local_row == 1 && !manager.pager && manager.confirm.is_none() && !manager.busy {
        // 2. 【后台任务】【范围命中】标签含实时计数，边界取已绘制筛选行的显示列
        let content = line.trim_start_matches(['│', ' ']);
        let mut start: usize = 3;
        let mut words = content.split_whitespace();
        for key in ['1', '2', '3'] {
            let number = words.next()?;
            let label = words.next()?;
            let count = words.next()?;
            let width =
                crate::render::ansi_clip::ansi_visible_width(&format!("{number} {label} {count}"));
            if column >= start.saturating_sub(1) && column <= start + width {
                return Some(KeyCode::Char(key));
            }
            start += width + 3;
        }
    }
    None
}
