//! 后台任务面板命令着色与多行展开排版：统一单行预览截断与完整多行折行高亮。

use crate::render::ansi_clip::{ansi_visible_width, clip_ansi_to_width, strip_ansi};
use crate::render::panel_palette::{fg, PanelPalette};
use crate::render::syntect_highlight::{paint, Carry};
use crate::render::transcript::AnsiLine;
use unicode_width::UnicodeWidthStr;

/// 【后台任务】【单行清洗】清除终端转义序列并将换行与制表符折叠为单个空格。
///
/// 参数:
/// - `command`: 原始命令字符串
///
/// 返回:
/// - 适合单行安全展示的纯文本命令
pub(super) fn sanitize_inline(command: &str) -> String {
    // 1. 【后台任务】【控制符剥离】先移除潜藏的 ANSI 序列，再将连续换行和制表符压缩成单空格
    let stripped = strip_ansi(command);
    let mut output = String::with_capacity(stripped.len());
    let mut pending_space = false;
    for ch in stripped.chars() {
        if matches!(ch, '\r' | '\n' | '\t') {
            pending_space = !output.is_empty();
        } else if ch.is_control() {
            continue;
        } else {
            if pending_space {
                if !output.ends_with(' ') {
                    output.push(' ');
                }
                pending_space = false;
            }
            output.push(ch);
        }
    }
    output.trim().to_string()
}

/// 【后台任务】【多行清洗】清除危险控制序列并保留真实换行与缩进结构。
///
/// 参数:
/// - `command`: 原始命令字符串
///
/// 返回:
/// - 保留逻辑行边界的安全多行文本
pub(super) fn sanitize_multiline(command: &str) -> String {
    // 1. 【后台任务】【换行归一】统一 CRLF/CR 为 LF，展开制表符并过滤其他控制字符
    let stripped = strip_ansi(command).replace("\r\n", "\n").replace('\r', "\n");
    let mut output = String::with_capacity(stripped.len());
    for ch in stripped.chars() {
        match ch {
            '\n' => output.push('\n'),
            '\t' => output.push_str("  "),
            _ if ch.is_control() => {}
            _ => output.push(ch),
        }
    }
    output.trim().to_string()
}

/// 【后台任务】【展开判定】判断命令是否包含多行或超出单行内联预算而需要展开查看。
///
/// 参数:
/// - `command`: 原始命令字符串
/// - `inline_budget`: 单行详情值区域的最大可见列数
///
/// 返回:
/// - 含真实换行或加上提示符后超出预算时返回 true
pub(super) fn needs_expansion(command: &str, inline_budget: usize) -> bool {
    let clean = sanitize_multiline(command);
    if clean.is_empty() {
        return false;
    }
    if clean.contains('\n') {
        return true;
    }
    2usize.saturating_add(UnicodeWidthStr::width(clean.as_str())) > inline_budget
}

/// 【后台任务】【单行命令着色】按 Bash 语法高亮命令并在超宽时附带省略号安全截断。
///
/// 参数:
/// - `command`: 原始命令字符串
/// - `width`: 目标最大可见列数
/// - `with_prompt`: 是否在行首附带强调色 `$ ` 提示符
/// - `palette`: 当前面板色板
///
/// 返回:
/// - 样式闭合且不超过 `width` 列的着色文本
pub(super) fn highlight_inline(
    command: &str,
    width: usize,
    with_prompt: bool,
    palette: &PanelPalette,
) -> String {
    if width == 0 {
        return String::new();
    }
    // 1. 【后台任务】【单行采样】清洗控制字符并限制解析上限，避免超长内联脚本阻塞渲染
    let clean = sanitize_inline(command);
    if clean.is_empty() {
        return String::new();
    }
    let char_cap = width.saturating_mul(16).max(2048);
    let sampled = match clean.char_indices().nth(char_cap) {
        Some((byte_idx, _)) => format!("{}…", &clean[..byte_idx]),
        None => clean,
    };
    // 2. 【后台任务】【语法上色】使用共享 Bash 语法引擎着色并按需拼接提示符后裁剪宽度
    let mut carry = None;
    let body = paint_bash_line(&sampled, &mut carry, palette);
    let styled = if with_prompt {
        format!("{}$ \x1b[0m{body}", fg(palette.accent))
    } else {
        body
    };
    if ansi_visible_width(&styled) <= width {
        styled
    } else {
        clip_ansi_to_width(&styled, width, "…")
    }
}

/// 【后台任务】【多行命令展开】保留真实换行并按可用列宽折行，跨行延续引号与语法高亮状态。
///
/// 参数:
/// - `command`: 原始命令字符串
/// - `width`: 命令正文区的最大可见列数
/// - `palette`: 当前面板色板
///
/// 返回:
/// - 首行带 `$ ` 提示符、续行带两空格对齐缩进的着色显示行列表
pub(super) fn highlight_expanded_lines(
    command: &str,
    width: usize,
    palette: &PanelPalette,
) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    let clean = sanitize_multiline(command);
    if clean.is_empty() {
        return vec![clip_ansi_to_width(
            &format!("{}-\x1b[0m", fg(palette.muted)),
            width,
            "",
        )];
    }
    if width <= 2 {
        return clean
            .split('\n')
            .map(|line| clip_ansi_to_width(line, width, ""))
            .collect();
    }
    // 1. 【后台任务】【跨行解析】逐逻辑行送入带真实换行的 Bash 解析器，保留多行字符串上下文
    let body_width = width.saturating_sub(2).max(1);
    let mut carry = None;
    let mut lines = Vec::new();
    for raw in clean.split('\n') {
        let highlighted = paint_bash_line(raw, &mut carry, palette);
        // 2. 【后台任务】【视觉折行】对已着色逻辑行做无损折行，首行加提示符，后续行对齐两列缩进
        for wrapped in AnsiLine::wrap_block(&highlighted, body_width) {
            let prefix = if lines.is_empty() {
                format!("{}$ \x1b[0m", fg(palette.accent))
            } else {
                "  ".to_string()
            };
            let combined = format!("{prefix}{}", wrapped.as_str());
            let fitted = if ansi_visible_width(&combined) <= width {
                combined
            } else {
                clip_ansi_to_width(&combined, width, "")
            };
            lines.push(fitted);
        }
    }
    lines
}

/// 【后台任务】【单逻辑行染色】向 syntect 提供带换行边界的源码行并兜底纯符号前景色。
///
/// 参数:
/// - `line`: 不含换行的单个逻辑行文本
/// - `carry`: 跨逻辑行复用的 syntect 会话
/// - `palette`: 当前面板色板
///
/// 返回:
/// - 不含换行符的 ANSI 着色字符串
fn paint_bash_line(
    line: &str,
    carry: &mut Option<Box<Carry>>,
    palette: &PanelPalette,
) -> String {
    if line.is_empty() {
        return String::new();
    }
    let source = format!("{line}\n");
    let painted = paint("bash", &source, carry)
        .map(|text| text.replace('\n', ""))
        .unwrap_or_else(|| line.to_string());
    if !painted.contains('\x1b') && !line.trim().is_empty() {
        format!("{}{painted}\x1b[0m", fg(palette.text))
    } else {
        painted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【后台任务测试】【内联着色】验证单行预览注入语法颜色、保留命令文本并在超宽时安全截断。
    /// 参数: 无；返回: 无。
    #[test]
    fn inline_highlighting_colors_tokens_and_clips_cleanly() {
        let palette = PanelPalette::for_background((20, 22, 24));
        let rendered = highlight_inline("echo 'hello world' && sleep 5", 80, true, &palette);
        assert!(rendered.contains("\x1b["), "{rendered:?}");
        assert_eq!(strip_ansi(&rendered), "$ echo 'hello world' && sleep 5");

        let clipped = highlight_inline("cargo test --workspace --all-targets", 16, true, &palette);
        assert!(ansi_visible_width(&clipped) <= 16);
        assert!(strip_ansi(&clipped).ends_with('…'));
    }

    /// 【后台任务测试】【展开折行】验证多行与超长命令完整保留所有参数且对齐首行与续行前缀。
    /// 参数: 无；返回: 无。
    #[test]
    fn expanded_lines_preserve_multiline_and_long_commands() {
        let palette = PanelPalette::for_background((20, 22, 24));
        let command = "export RUST_LOG=info\ncargo run --bin sai -- --port 8080";
        assert!(needs_expansion(command, 80));
        assert!(!needs_expansion("echo ok", 20));
        assert!(needs_expansion("echo very-long-argument-list", 12));

        let lines = highlight_expanded_lines(command, 24, &palette);
        assert!(lines.len() >= 3, "{lines:?}");
        assert!(strip_ansi(&lines[0]).starts_with("$ export"));
        assert!(lines[1..].iter().all(|line| strip_ansi(line).starts_with("  ")));
        assert!(lines.iter().all(|line| ansi_visible_width(line) <= 24));
        let joined = lines
            .iter()
            .map(|line| strip_ansi(line))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(joined.contains("RUST_LOG=info") && joined.contains("8080"), "{joined}");
    }
}
