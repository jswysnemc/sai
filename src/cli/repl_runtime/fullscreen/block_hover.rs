//! 正文区块悬停背景，不改变文字、语法颜色与左侧引导线前景。

use crate::render::panel_palette::{bg, PanelPalette};

/// 【正文悬停】【背景叠加】为默认背景添加浅色底，保留已有差异背景和全部前景样式。
/// 参数: line 为已经补齐宽度的正文行；返回: 带区块背景的 ANSI 行。
pub(super) fn tint(line: &str) -> String {
    let background = bg(PanelPalette::current().hover_bg);
    let mut output = background.clone();
    let mut index = 0;
    while index < line.len() {
        if line.as_bytes()[index] == 0x1b {
            let end = crate::render::terminal_image::escape_sequence_end(line, index)
                .max(index + 1)
                .min(line.len());
            let sequence = &line[index..end];
            output.push_str(sequence);
            // 1. 【正文悬停】【复位恢复】只在最终背景恢复默认时补底，不把颜色分量误判为命令
            if resets_background(sequence) {
                output.push_str(&background);
            }
            index = end;
        } else {
            let ch = line[index..].chars().next().unwrap();
            output.push(ch);
            index += ch.len_utf8();
        }
    }
    output.push_str("\x1b[0m");
    output
}

/// 【正文悬停】【背景状态】解析复合 SGR 中最后一次背景操作，跳过扩展颜色分量。
/// 参数: sequence 为单个控制序列；返回: 是否恢复了默认背景。
fn resets_background(sequence: &str) -> bool {
    let Some(params) = sequence
        .strip_prefix("\x1b[")
        .and_then(|s| s.strip_suffix('m'))
    else {
        return false;
    };
    let mut parts = params.split(';');
    let mut reset = false;
    while let Some(part) = parts.next() {
        let code = part.split(':').next().unwrap_or("");
        match code {
            "" | "0" | "49" => reset = true,
            "38" | "48" | "58" => {
                if code == "48" {
                    reset = false;
                }
                if !part.contains(':') {
                    let skip = match parts.next() {
                        Some("2") => 3,
                        Some("5") => 1,
                        _ => 0,
                    };
                    for _ in 0..skip {
                        parts.next();
                    }
                }
            }
            other => {
                if other
                    .parse::<u16>()
                    .ok()
                    .is_some_and(|v| matches!(v, 40..=47 | 100..=107))
                {
                    reset = false;
                }
            }
        }
    }
    reset
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【正文悬停】【颜色回归】前景、暗淡属性和图片协议不变，复位后恢复背景。
    /// 参数: 无；返回: 无。
    #[test]
    fn preserves_foregrounds_and_graphics() {
        let line = "\x1b[2m\x1b[38;2;0;49;48m│\x1b[0mtext\x1b[49m tail\x1b_Ga=p,i=4,p=2\x1b\\";
        let output = tint(line);
        assert!(output.contains("\x1b[2m\x1b[38;2;0;49;48m│"));
        assert!(output.contains("\x1b_Ga=p,i=4,p=2\x1b\\"));
        let background = bg(PanelPalette::current().hover_bg);
        assert!(output.contains(&format!("\x1b[0m{background}text")));
        assert!(output.contains(&format!("\x1b[49m{background} tail")));
        assert!(!resets_background("\x1b[38;2;0;49;48m"));
        assert!(!resets_background("\x1b[0;48;5;22m"));
        assert!(resets_background("\x1b[48;5;22;49m"));
        assert!(!resets_background("\x1b[48:2::0:49:48m"));
    }
}
