use crate::render::code_block::highlight_code_line;
use std::path::Path;

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

/// 给 diff 上下文行添加样式。
///
/// 参数:
/// - `path`: 文件路径
/// - `line`: diff 行文本
///
/// 返回:
/// - 带 ANSI 样式的上下文行
pub(crate) fn style_context_line(path: &Path, line: &str) -> String {
    highlight_code_line(language_from_path(path), line)
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
    let highlighted = highlight_code_line(language_from_path(path), line);
    let highlighted = keep_diff_background_after_reset(&highlighted, background, foreground);
    // `\x1b[K` = Erase to end of line，在已设置的背景下铺满本行剩余列
    format!(
        "\x1b[48;5;{background}m\x1b[38;5;{foreground}m{highlighted}\x1b[48;5;{background}m\x1b[K\x1b[0m"
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

    #[test]
    fn context_lines_keep_terminal_default_background() {
        let output = style_context_line(Path::new("hello.rs"), "  1    fn main() {}");

        assert!(!output.contains("\x1b[48;5;"));
        assert!(!output.contains("\x1b[K"));
    }
}
