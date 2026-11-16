//! 底栏和队列控件的局部前景高亮，与文本拖选的反色样式分开。

use super::sidebar_palette::{fg, SidebarPalette};

/// 【终端交互】【局部高亮】参数：line 为 ANSI 行，from/to 为显示列范围；返回：仅目标文字使用主题色的行
pub(in crate::cli) fn tint_cols(line: &str, from: usize, to: usize) -> String {
    let style = format!("\x1b[22m{}", fg(SidebarPalette::current().accent));
    let mut original_style = String::new();
    let mut output = String::new();
    let mut index = 0;
    let mut column = 0;
    let mut active = false;
    // 1. 【终端交互】【显示列定位】跳过控制序列，宽字符与目标区域重叠时完整高亮
    while index < line.len() {
        if line.as_bytes()[index] == 0x1b {
            let end = crate::render::terminal_image::escape_sequence_end(line, index)
                .max(index + 1)
                .min(line.len());
            let sequence = &line[index..end];
            crate::render::ansi_style::update_active_sgr(&mut original_style, sequence);
            output.push_str(sequence);
            if active {
                output.push_str(&style);
            }
            index = end;
            continue;
        }
        let ch = line[index..].chars().next().unwrap();
        let width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        let inside = (column >= from || (width > 1 && column + width > from)) && column < to;
        if inside && !active {
            output.push_str(&style);
        } else if !inside && active {
            output.push_str(&format!("\x1b[0m{original_style}"));
        }
        active = inside;
        output.push(ch);
        column += width;
        index += ch.len_utf8();
    }
    // 2. 【终端交互】【原样式恢复】离开目标范围后恢复原前景和文字属性
    if active {
        output.push_str(&format!("\x1b[0m{original_style}"));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【终端交互测试】【局部颜色】参数：无；返回：无，验证宽字符、颜色复位与文字宽度，不增加背景
    #[test]
    fn hover_range_preserves_text_and_restores_neighbor_styles() {
        let line = "\x1b[37m模型\x1b[0m high rest";
        let result = tint_cols(line, 1, 8);
        assert_eq!(
            crate::render::ansi_clip::strip_ansi(&result),
            "模型 high rest"
        );
        assert!(result.contains(&fg(SidebarPalette::current().accent)));
        assert!(!result.contains("\x1b[7m"));
        assert!(!result.contains("\x1b[48;"));
    }
}
