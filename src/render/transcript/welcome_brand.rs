//! 全屏欢迎的紧凑品牌区：原生标志、会话信息和权限状态。

use super::line::AnsiLine;
use super::welcome_cell::WelcomeCell;
use crate::render::ansi_clip::{ansi_visible_width, clip_ansi_to_width};
use crate::render::brand_logo::{logo_lines, LOGO_WIDTH, MARK_STYLE};

const MUTED: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";
const GAP: usize = 2;
const MAX_CONTENT_WIDTH: usize = 48;

/// 【终端欢迎】【品牌排版】宽屏使用三行原生标志，窄屏去掉标志以保留模型和目录。
/// 参数: cell 为会话信息，width 为正文净宽；返回: 已适配宽度且不含背景的行。
pub(super) fn render(cell: &WelcomeCell, width: usize) -> Vec<AnsiLine> {
    let width = width.max(1);
    let show_logo = width >= 40;
    let inset = if show_logo { LOGO_WIDTH + GAP } else { 0 };
    let content_width = width.saturating_sub(inset).clamp(1, MAX_CONTENT_WIDTH);
    // 1. 【终端欢迎】【标题层次】品牌名强调，版本弱化，权限只使用已有风险色
    let name = format!("\x1b[1msai{RESET}  {MUTED}v{}{RESET}", cell.version);
    let mode = format!(
        "{}{}{RESET}",
        super::welcome_cell::permission_style(&cell.permissions),
        cell.permissions
    );
    let gap = content_width.saturating_sub(ansi_visible_width(&name) + ansi_visible_width(&mode));
    let title = if gap >= 2 {
        format!("{name}{}{mode}", " ".repeat(gap))
    } else {
        name
    };
    let mut rows = AnsiLine::wrap_block(&title, content_width);
    if gap < 2 {
        rows.extend(AnsiLine::wrap_block(&mode, content_width));
    }
    // 2. 【终端欢迎】【信息换行】真实值完整换行，不用省略号丢弃模型名称或工作目录
    rows.extend(AnsiLine::wrap_block(
        &format!("{MUTED}model{RESET} {}", cell.model),
        content_width,
    ));
    rows.extend(AnsiLine::wrap_block(
        &format!("{MUTED}dir{RESET}   {MUTED}{}{RESET}", cell.directory),
        content_width,
    ));
    let logo = if show_logo {
        logo_lines(MARK_STYLE)
    } else {
        Vec::new()
    };
    rows.into_iter()
        .enumerate()
        .map(|(index, row)| {
            let mark = if show_logo {
                format!(
                    "{}{}",
                    logo.get(index)
                        .cloned()
                        .unwrap_or_else(|| " ".repeat(LOGO_WIDTH)),
                    " ".repeat(GAP)
                )
            } else {
                String::new()
            };
            AnsiLine::new(clip_ansi_to_width(
                &format!("{mark}{}", row.as_str()),
                width,
                "",
            ))
        })
        .collect()
}
