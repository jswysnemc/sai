use crate::render::code_block::highlight_code_line;
use std::path::Path;

/// 行号列前景色。
///
/// 行号是定位坐标而不是内容，需要能扫到但不该与代码争夺注意力。
/// 早先整行连同行号一起交给代码高亮器，行号被当成数字字面量着色，
/// 于是和代码里的数字同色——一列本该保持中性的坐标跟着语法变色。
const LINE_NUMBER_COLOR: u8 = 244;

/// 增删标记的前景色。
///
/// 标记与行号同属"这行发生了什么"的坐标信息，但需要比行号更醒目一档，
/// 因此取行背景对应的前景色，由调用方按增删分别传入。
const MARKER_DIM: u8 = 250;

/// diff 行配色。
#[derive(Debug, Clone, Copy)]
pub(crate) struct DiffPalette {
    pub delete_background: u8,
    pub delete_foreground: u8,
    pub add_background: u8,
    pub add_foreground: u8,
}

impl Default for DiffPalette {
    /// 构造默认 diff 配色。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 默认深色终端配色
    fn default() -> Self {
        Self {
            delete_background: 52,
            delete_foreground: 174,
            add_background: 22,
            add_foreground: 108,
        }
    }
}

/// 拆出 diff 行的行号列与代码正文。
///
/// 行首结构固定为「右对齐行号 空格 标记 两个空格 正文」，标记为 `+`、`-` 或空格。
/// 拆开后行号与标记走固定的中性色，正文才交给语法高亮，
/// 否则行号会被高亮器当作数字字面量，和代码里的字面量同色。
///
/// 参数:
/// - `line`: 完整的 diff 行文本
///
/// 返回:
/// - `(行号与标记前缀, 代码正文)`；不符合前缀结构时前缀为空
fn split_line_number(line: &str) -> (&str, &str) {
    let digits_end = line
        .find(|ch: char| !ch.is_ascii_whitespace())
        .filter(|start| line[*start..].starts_with(|ch: char| ch.is_ascii_digit()))
        .map(|start| {
            start
                + line[start..]
                    .find(|ch: char| !ch.is_ascii_digit())
                    .unwrap_or(line.len() - start)
        });
    let Some(digits_end) = digits_end else {
        return ("", line);
    };
    // 行号之后必须是「空格 标记 两个空格」，否则说明这行不是标准 diff 正文
    let rest = &line[digits_end..];
    let Some(after_marker) = rest
        .strip_prefix(' ')
        .filter(|value| value.starts_with(['+', '-', ' ']))
        .map(|value| &value[1..])
    else {
        return ("", line);
    };
    let Some(body) = after_marker.strip_prefix("  ") else {
        return ("", line);
    };
    let prefix_len = line.len() - body.len();
    (&line[..prefix_len], body)
}

/// 给 diff 上下文行添加样式。
///
/// 行首以一个空样式序列开头、行尾附加 `EL`，使上下文行与增删行的 ANSI
/// 前缀结构一致。否则代码高亮器会把行号右对齐的填充空格留在样式序列之前，
/// 而增删行的同一批空格位于背景序列之后，两者在引导对齐时相差一列。
///
/// 参数:
/// - `path`: 文件路径
/// - `line`: diff 行文本
///
/// 返回:
/// - 带 ANSI 样式的上下文行
pub(crate) fn style_context_line(path: &Path, line: &str) -> String {
    let (prefix, body) = split_line_number(line);
    if prefix.is_empty() {
        return format!(
            "\x1b[m{}\x1b[K",
            highlight_code_line(language_from_path(path), line)
        );
    }
    format!(
        "\x1b[m\x1b[38;5;{LINE_NUMBER_COLOR}m{prefix}\x1b[0m{}\x1b[K",
        highlight_code_line(language_from_path(path), body)
    )
}

/// 给 diff 删除行添加样式。
///
/// 参数:
/// - `path`: 文件路径
/// - `line`: diff 行文本
///
/// 返回:
/// - 带 ANSI 样式的删除行
pub(crate) fn style_removed_line(path: &Path, line: &str) -> String {
    let palette = DiffPalette::default();
    style_diff_line(
        path,
        line,
        palette.delete_background,
        palette.delete_foreground,
    )
}

/// 给 diff 新增行添加样式。
///
/// 参数:
/// - `path`: 文件路径
/// - `line`: diff 行文本
///
/// 返回:
/// - 带 ANSI 样式的新增行
pub(crate) fn style_added_line(path: &Path, line: &str) -> String {
    let palette = DiffPalette::default();
    style_diff_line(path, line, palette.add_background, palette.add_foreground)
}

/// 给新增行数添加样式。
///
/// 参数:
/// - `count`: 新增行数
///
/// 返回:
/// - 带 ANSI 样式的新增行数
pub(crate) fn style_added_count(count: usize) -> String {
    format!("\x1b[32m+{count}\x1b[0m")
}

/// 给删除行数添加样式。
///
/// 参数:
/// - `count`: 删除行数
///
/// 返回:
/// - 带 ANSI 样式的删除行数
pub(crate) fn style_removed_count(count: usize) -> String {
    format!("\x1b[31m-{count}\x1b[0m")
}

/// 给 diff 行添加背景和代码高亮。
///
/// 行号与增删标记单独着中性色，只有代码正文交给语法高亮：
/// 整行一起高亮会让行号被当成数字字面量，跟着代码里的字面量一起变色。
///
/// 背景铺满策略：
/// - 不使用「按当前终端宽度填充空格」。缩放后 scrollback reflow 会把
///   带背景的空格拆成错位色块（见终端改宽后的碎绿条）。
/// - 在内容后以当前背景执行 `EL`（擦到行尾），由终端把剩余列画成背景，
///   不把固定宽度写进缓冲区。
///
/// 参数:
/// - `path`: 文件路径，用于推断语言
/// - `line`: diff 行文本
/// - `background`: ANSI 256 色背景
/// - `foreground`: ANSI 256 色前景
///
/// 返回:
/// - 带 ANSI 样式的 diff 行
fn style_diff_line(path: &Path, line: &str, background: u8, foreground: u8) -> String {
    let (prefix, body) = split_line_number(line);
    let highlighted = highlight_code_line(language_from_path(path), body);
    let highlighted = keep_diff_background_after_reset(&highlighted, background, foreground);
    // 行号列在同一背景上压暗，代码正文回到本行前景色
    let numbered = if prefix.is_empty() {
        highlighted
    } else {
        format!("\x1b[38;5;{MARKER_DIM}m{prefix}\x1b[38;5;{foreground}m{highlighted}")
    };
    // `\x1b[K` = Erase to end of line，在已设置的背景下铺满本行剩余列
    format!(
        "\x1b[48;5;{background}m\x1b[38;5;{foreground}m{numbered}\x1b[48;5;{background}m\x1b[K\x1b[0m"
    )
}

/// 在代码高亮 reset 后恢复 diff 背景。
///
/// 参数:
/// - `text`: 已高亮的 diff 行
/// - `background`: ANSI 256 色背景
/// - `foreground`: ANSI 256 色前景
///
/// 返回:
/// - reset 后重新应用背景和前景的文本
fn keep_diff_background_after_reset(text: &str, background: u8, foreground: u8) -> String {
    text.replace(
        "\x1b[0m",
        &format!("\x1b[0m\x1b[48;5;{background}m\x1b[38;5;{foreground}m"),
    )
}

/// 根据文件路径推断代码高亮语言。
///
/// 参数:
/// - `path`: 文件路径
///
/// 返回:
/// - 代码高亮语言标识
fn language_from_path(path: &Path) -> &str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
    {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" => "javascript",
        "py" => "python",
        "sh" | "bash" | "zsh" => "sh",
        "json" => "json",
        "toml" => "toml",
        "md" => "markdown",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restores_background_after_syntax_reset() {
        let output = style_added_line(Path::new("main.rs"), "  1 +  fn main() {}");

        assert!(output.contains("\x1b[0m\x1b[48;5;22m\x1b[38;5;108m"));
    }

    #[test]
    fn fills_line_with_erase_not_space_padding() {
        let output = style_added_line(Path::new("hello.txt"), "  1 +  hello");
        // 用 EL 铺满，避免写入与终端等宽的大量空格
        assert!(output.contains("\x1b[K"));
        assert!(!output.contains(&" ".repeat(40)));
        assert!(output.ends_with("\x1b[0m") || output.contains("\x1b[K\x1b[0m"));
    }

    /// 上下文行不设背景色，但保留与增删行一致的 ANSI 前后缀结构。
    #[test]
    fn context_lines_keep_terminal_default_background() {
        let output = style_context_line(Path::new("hello.rs"), "  1    fn main() {}");

        assert!(!output.contains("\x1b[48;5;"));
        // 前缀与行尾序列用于和增删行对齐，不引入任何颜色
        assert!(output.starts_with("\x1b[m"));
        assert!(output.ends_with("\x1b[K"));
    }

    /// 行号列走固定中性色，不跟随代码里的数字字面量着色。
    #[test]
    fn line_numbers_use_a_dedicated_neutral_color() {
        let context = style_context_line(Path::new("main.rs"), " 12    let total = 42;");

        // 行号紧跟专属前景色，与正文里的 42 分属不同样式
        assert!(context.contains(&format!("\x1b[38;5;{LINE_NUMBER_COLOR}m 12    ")));
    }

    /// 增删行的行号列压暗，代码正文回到本行前景色。
    #[test]
    fn changed_line_numbers_are_dimmed_within_the_row_background() {
        let palette = DiffPalette::default();
        let added = style_added_line(Path::new("main.rs"), "  7 +  let total = 42;");

        assert!(added.contains(&format!("\x1b[38;5;{MARKER_DIM}m  7 +  ")));
        // 行号之后立即恢复本行前景色，正文不沿用压暗色
        assert!(added.contains(&format!(
            "\x1b[38;5;{MARKER_DIM}m  7 +  \x1b[38;5;{}m",
            palette.add_foreground
        )));
    }

    /// 不符合行号前缀结构的行整体交给语法高亮，不做拆分。
    #[test]
    fn lines_without_a_number_prefix_are_highlighted_as_a_whole() {
        let output = style_context_line(Path::new("main.rs"), "@@ hunk header");

        assert!(!output.contains(&format!("\x1b[38;5;{LINE_NUMBER_COLOR}m")));
    }
}
