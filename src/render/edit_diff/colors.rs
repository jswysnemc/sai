use crate::render::code_block::highlight_code_line;
use crate::render::panel_palette::PanelPalette;
use crate::render::terminal_contrast::{readable_color_on, Rgb, MIN_TEXT_CONTRAST};
use crate::render::terminal_palette::terminal_palette;
use std::path::Path;

/// 行号列前景色。
///
/// 行号是定位坐标而不是内容，需要能扫到但不该与代码争夺注意力。
/// 早先整行连同行号一起交给代码高亮器，行号被当成数字字面量着色，
/// 于是和代码里的数字同色——一列本该保持中性的坐标跟着语法变色。
const LINE_NUMBER_COLOR: u8 = 244;

/// 增删行内行号列的前景色，在深色底上要比上下文行号亮一档才可读。
const FILL_LINE_NUMBER: u8 = 250;

/// 兼容 256 色与 24 位真彩的 diff 色彩单元。
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum DiffColor {
    Ansi256(u8),
    Rgb(Rgb),
}

impl DiffColor {
    /// 【差异配色】【前景序列】输出对应色彩能力的前景 SGR 序列。
    ///
    /// 参数:
    /// - `self`: 颜色配置
    ///
    /// 返回:
    /// - ANSI 前景色控制序列
    pub(crate) fn fg(self) -> String {
        match self {
            Self::Ansi256(index) => format!("\x1b[38;5;{index}m"),
            Self::Rgb((r, g, b)) => format!("\x1b[38;2;{r};{g};{b}m"),
        }
    }

    /// 【差异配色】【背景序列】输出对应色彩能力的背景 SGR 序列。
    ///
    /// 参数:
    /// - `self`: 颜色配置
    ///
    /// 返回:
    /// - ANSI 背景色控制序列
    pub(crate) fn bg(self) -> String {
        match self {
            Self::Ansi256(index) => format!("\x1b[48;5;{index}m"),
            Self::Rgb((r, g, b)) => format!("\x1b[48;2;{r};{g};{b}m"),
        }
    }

    /// 【差异配色】【RGB 换算】提取颜色对应的近似 RGB 用于对比度校正。
    ///
    /// 参数:
    /// - `self`: 颜色配置
    ///
    /// 返回:
    /// - 对应的 RGB 元组
    fn to_rgb(self) -> Rgb {
        match self {
            Self::Rgb(rgb) => rgb,
            Self::Ansi256(index) => {
                crate::render::terminal_text_style::palette_rgb(index).unwrap_or((128, 128, 128))
            }
        }
    }
}

/// diff 行配色。
#[derive(Debug, Clone, Copy)]
pub(crate) struct DiffPalette {
    pub light: bool,
    pub true_color: bool,
    pub terminal_background: Rgb,
    pub text_foreground: Rgb,
    pub context_line_number: DiffColor,
    pub fill_line_number: DiffColor,
    pub delete_background: DiffColor,
    pub delete_foreground: DiffColor,
    pub add_background: DiffColor,
    pub add_foreground: DiffColor,
    /// 删除行内被替换片段的强调背景（比整行底色更突出一档）
    pub delete_emphasis_background: DiffColor,
    /// 新增行内替换片段的强调背景
    pub add_emphasis_background: DiffColor,
}

impl Default for DiffPalette {
    /// 【差异配色】【默认深色】构造经典深色终端 diff 配色。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 默认深色终端配色
    fn default() -> Self {
        Self {
            light: false,
            true_color: false,
            terminal_background: (24, 28, 32),
            text_foreground: (214, 218, 222),
            context_line_number: DiffColor::Ansi256(LINE_NUMBER_COLOR),
            fill_line_number: DiffColor::Ansi256(FILL_LINE_NUMBER),
            delete_background: DiffColor::Ansi256(52),
            delete_foreground: DiffColor::Ansi256(174),
            add_background: DiffColor::Ansi256(22),
            add_foreground: DiffColor::Ansi256(108),
            delete_emphasis_background: DiffColor::Ansi256(88),
            add_emphasis_background: DiffColor::Ansi256(28),
        }
    }
}

impl DiffPalette {
    /// 【差异配色】【环境自适应】按当前缓存的终端背景明暗选择深浅 diff 主题。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 适配当前终端背景的 diff 调色板
    pub(crate) fn current() -> Self {
        let terminal = terminal_palette();
        Self::for_background(terminal.background, terminal.true_color)
    }

    /// 【差异配色】【明暗构建】深色背景保持经典 256 色方案，浅色背景使用柔和浅底与深色字。
    ///
    /// 参数:
    /// - `background`: 终端背景 RGB
    /// - `true_color`: 终端是否支持 24 位真彩
    ///
    /// 返回:
    /// - 对应明暗主题的 diff 调色板
    pub(crate) fn for_background(background: Rgb, true_color: bool) -> Self {
        let (r, g, b) = background;
        let light = u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114 > 128_000;
        if !light {
            return Self {
                true_color,
                terminal_background: background,
                ..Self::default()
            };
        }
        // 1. 【差异配色】【浅色面板】复用面板调色板派生浅绿浅红底色与可读深色前景
        let panel = PanelPalette::for_background(background);
        if true_color {
            let add_bg = panel.added_bg;
            let del_bg = panel.removed_bg;
            Self {
                light: true,
                true_color: true,
                terminal_background: background,
                text_foreground: panel.text,
                context_line_number: DiffColor::Rgb(panel.muted),
                fill_line_number: DiffColor::Rgb(readable_color_on(
                    panel.muted,
                    add_bg,
                    MIN_TEXT_CONTRAST,
                )),
                delete_background: DiffColor::Rgb(del_bg),
                delete_foreground: DiffColor::Rgb(readable_color_on(
                    panel.removed,
                    del_bg,
                    MIN_TEXT_CONTRAST,
                )),
                add_background: DiffColor::Rgb(add_bg),
                add_foreground: DiffColor::Rgb(readable_color_on(
                    panel.added,
                    add_bg,
                    MIN_TEXT_CONTRAST,
                )),
                delete_emphasis_background: DiffColor::Rgb((245, 198, 202)),
                add_emphasis_background: DiffColor::Rgb((192, 228, 204)),
            }
        } else {
            // 2. 【差异配色】【256 色降级】无真彩能力时采用柔和浅绿/浅红 256 色号
            Self {
                light: true,
                true_color: false,
                terminal_background: background,
                text_foreground: panel.text,
                context_line_number: DiffColor::Ansi256(242),
                fill_line_number: DiffColor::Ansi256(240),
                delete_background: DiffColor::Ansi256(224),
                delete_foreground: DiffColor::Ansi256(124),
                add_background: DiffColor::Ansi256(194),
                add_foreground: DiffColor::Ansi256(28),
                delete_emphasis_background: DiffColor::Ansi256(217),
                add_emphasis_background: DiffColor::Ansi256(151),
            }
        }
    }

    /// 【差异配色】【正文前景】返回代码正文重置为默认可读色时的前缀序列。
    ///
    /// 参数:
    /// - `self`: 当前调色板
    ///
    /// 返回:
    /// - 深色模式返回 `\x1b[39m`，浅色模式返回显式深色前景
    fn body_foreground_sequence(self) -> String {
        if !self.light {
            return "\x1b[39m".to_string();
        }
        if self.true_color {
            let (r, g, b) = self.text_foreground;
            format!("\x1b[38;2;{r};{g};{b}m")
        } else {
            "\x1b[38;5;237m".to_string()
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

/// 【差异配色】【上下文行】给 diff 上下文行添加明暗自适应样式。
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
    style_context_line_with_palette(path, line, DiffPalette::current())
}

/// 【差异配色】【上下文行构建】按指定调色板渲染 diff 上下文行。
///
/// 参数:
/// - `path`: 文件路径
/// - `line`: diff 行文本
/// - `palette`: 当前明暗调色板
///
/// 返回:
/// - 带 ANSI 样式的上下文行
fn style_context_line_with_palette(path: &Path, line: &str, palette: DiffPalette) -> String {
    let (prefix, body) = split_line_number(line);
    let target = if prefix.is_empty() { line } else { body };
    let mut highlighted = highlight_code_line(language_from_path(path), target);
    // 1. 【差异配色】【浅色深字】浅色背景校正代码 token 对比度，避免浅色字面量在白底隐形
    if palette.light {
        highlighted = crate::render::terminal_text_style::correct_ansi_text(
            &highlighted,
            Some((palette.text_foreground, palette.terminal_background)),
            palette.true_color,
        );
    }
    if prefix.is_empty() {
        return format!("\x1b[m{highlighted}\x1b[K");
    }
    let number_fg = palette.context_line_number.fg();
    format!("\x1b[m{number_fg}{prefix}\x1b[0m{highlighted}\x1b[K")
}

/// 【差异配色】【删除行】给 diff 删除行添加明暗自适应样式，可选高亮行内被替换片段。
///
/// 参数:
/// - `path`: 文件路径
/// - `line`: diff 行文本
/// - `emphasis`: 正文中被替换片段的字符区间
///
/// 返回:
/// - 带 ANSI 样式的删除行
pub(crate) fn style_removed_line_ranged(
    path: &Path,
    line: &str,
    emphasis: Option<std::ops::Range<usize>>,
) -> String {
    style_removed_line_with_palette(path, line, emphasis, DiffPalette::current())
}

/// 【差异配色】【删除行构建】按指定调色板渲染 diff 删除行。
///
/// 参数:
/// - `path`: 文件路径
/// - `line`: diff 行文本
/// - `emphasis`: 正文中被替换片段的字符区间
/// - `palette`: 当前明暗调色板
///
/// 返回:
/// - 带 ANSI 样式的删除行
fn style_removed_line_with_palette(
    path: &Path,
    line: &str,
    emphasis: Option<std::ops::Range<usize>>,
    palette: DiffPalette,
) -> String {
    style_diff_line(
        path,
        line,
        palette,
        palette.delete_background,
        palette.delete_foreground,
        emphasis.map(|range| (range, palette.delete_emphasis_background)),
    )
}

/// 【差异配色】【新增行】给 diff 新增行添加明暗自适应样式。
///
/// 参数:
/// - `path`: 文件路径
/// - `line`: diff 行文本
///
/// 返回:
/// - 带 ANSI 样式的新增行
pub(crate) fn style_added_line(path: &Path, line: &str) -> String {
    style_added_line_ranged(path, line, None)
}

/// 【差异配色】【新增行区间】给 diff 新增行添加明暗自适应样式，可选高亮行内替换片段。
///
/// 参数:
/// - `path`: 文件路径
/// - `line`: diff 行文本
/// - `emphasis`: 正文中替换片段的字符区间
///
/// 返回:
/// - 带 ANSI 样式的新增行
pub(crate) fn style_added_line_ranged(
    path: &Path,
    line: &str,
    emphasis: Option<std::ops::Range<usize>>,
) -> String {
    style_added_line_with_palette(path, line, emphasis, DiffPalette::current())
}

/// 【差异配色】【新增行构建】按指定调色板渲染 diff 新增行。
///
/// 参数:
/// - `path`: 文件路径
/// - `line`: diff 行文本
/// - `emphasis`: 正文中替换片段的字符区间
/// - `palette`: 当前明暗调色板
///
/// 返回:
/// - 带 ANSI 样式的新增行
fn style_added_line_with_palette(
    path: &Path,
    line: &str,
    emphasis: Option<std::ops::Range<usize>>,
    palette: DiffPalette,
) -> String {
    style_diff_line(
        path,
        line,
        palette,
        palette.add_background,
        palette.add_foreground,
        emphasis.map(|range| (range, palette.add_emphasis_background)),
    )
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

/// 【差异配色】【变更行渲染】给 diff 行添加背景和代码高亮。
///
/// 参数:
/// - `path`: 文件路径，用于推断语言
/// - `line`: diff 行文本
/// - `palette`: 当前明暗调色板
/// - `background`: 行整行底色
/// - `marker_color`: 行首增删标记的前景色
/// - `emphasis`: 可选的行内强调（正文字符区间 + 强调背景色）
///
/// 返回:
/// - 带 ANSI 样式的 diff 行
fn style_diff_line(
    path: &Path,
    line: &str,
    palette: DiffPalette,
    background: DiffColor,
    marker_color: DiffColor,
    emphasis: Option<(std::ops::Range<usize>, DiffColor)>,
) -> String {
    let (prefix, body) = split_line_number(line);
    let mut highlighted = highlight_code_line(language_from_path(path), body);
    // 1. 【差异配色】【浅色语法校正】在柔和浅绿/浅红底色上将语法高亮校正为高对比度深色字
    if palette.light {
        highlighted = crate::render::terminal_text_style::correct_ansi_text(
            &highlighted,
            Some((palette.text_foreground, background.to_rgb())),
            palette.true_color,
        );
    }
    let body_fg = palette.body_foreground_sequence();
    let reset_restore_fg = if palette.light { body_fg.as_str() } else { "" };
    // 2. 【差异配色】【行内底色恢复】按是否存在替换区间恢复整行底色或切换强调底色
    let highlighted = match &emphasis {
        Some((range, strong)) if !range.is_empty() => apply_intraline_background(
            &highlighted,
            range,
            background,
            *strong,
            reset_restore_fg,
        ),
        _ => keep_diff_background_after_reset(&highlighted, background, reset_restore_fg),
    };
    // 3. 【差异配色】【行号标记拼装】行首前缀拆出行号列与增删标记，再切回正文前景
    let numbered = if prefix.len() >= 3 {
        let (number_part, marker_part) = prefix.split_at(prefix.len() - 3);
        format!(
            "{}{number_part}{}{marker_part}{body_fg}{highlighted}",
            palette.fill_line_number.fg(),
            marker_color.fg(),
        )
    } else {
        format!(
            "{}{highlighted}",
            if palette.light { body_fg.as_str() } else { "" }
        )
    };
    let bg_seq = background.bg();
    format!("{bg_seq}{numbered}{bg_seq}\x1b[K\x1b[0m")
}

/// 【差异配色】【复位续航】在代码高亮 reset 后恢复 diff 背景（浅色主题同步恢复深色正文前景）。
///
/// 参数:
/// - `text`: 已高亮的 diff 行
/// - `background`: 行底色
/// - `restore_fg`: 浅色主题需要同步恢复的深色前景序列，深色主题为空串
///
/// 返回:
/// - reset 后重新应用背景的文本
fn keep_diff_background_after_reset(
    text: &str,
    background: DiffColor,
    restore_fg: &str,
) -> String {
    let bg_seq = background.bg();
    text.replace("\x1b[0m", &format!("\x1b[0m{bg_seq}{restore_fg}"))
}

/// 【差异配色】【行内强调】给已高亮正文的指定可见字符区间套上强调背景。
///
/// 参数:
/// - `text`: 已语法高亮的正文
/// - `range`: 强调的可见字符区间（半开）
/// - `base`: 整行底色
/// - `strong`: 强调背景色
/// - `restore_fg`: 浅色主题在 reset 后恢复的深色前景序列
///
/// 返回:
/// - 注入背景切换序列后的正文
fn apply_intraline_background(
    text: &str,
    range: &std::ops::Range<usize>,
    base: DiffColor,
    strong: DiffColor,
    restore_fg: &str,
) -> String {
    let base_seq = base.bg();
    let strong_seq = strong.bg();
    let mut output = String::with_capacity(text.len() + 64);
    let mut visible = 0usize;
    let mut index = 0usize;
    let mut in_emphasis = false;
    while index < text.len() {
        if text[index..].starts_with('\x1b') {
            let end =
                crate::render::terminal_image::escape_sequence_end(text, index).max(index + 1);
            let sequence = &text[index..end];
            output.push_str(sequence);
            // 1. 【差异配色】【语法复位】reset 清掉背景后按当前所在区间恢复对应底色
            if sequence == "\x1b[0m" {
                let current = if in_emphasis { &strong_seq } else { &base_seq };
                output.push_str(current);
                output.push_str(restore_fg);
            }
            index = end;
            continue;
        }
        // 2. 【差异配色】【区间切换】进入或离开替换区间时在可见字符前切换底色
        if visible == range.start && !in_emphasis {
            output.push_str(&strong_seq);
            in_emphasis = true;
        }
        if visible == range.end && in_emphasis {
            output.push_str(&base_seq);
            in_emphasis = false;
        }
        let ch = text[index..].chars().next().unwrap_or_default();
        output.push(ch);
        visible += 1;
        index += ch.len_utf8();
    }
    // 3. 【差异配色】【行尾收敛】强调区间延伸至行尾时切回整行底色，保证 EL 铺满一致
    if in_emphasis {
        output.push_str(&base_seq);
    }
    output
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
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "py" => "python",
        "sh" | "bash" | "zsh" => "sh",
        "json" | "jsonc" => "json",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "css" | "scss" | "less" => "css",
        "html" | "htm" | "vue" | "svelte" => "html",
        "c" | "h" => "c",
        "cpp" | "cc" | "hpp" => "cpp",
        "go" => "go",
        "java" => "java",
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

        // 语法 token 收尾后只恢复背景，前景回到默认，与上下文行同色
        assert!(output.contains("\x1b[0m\x1b[48;5;22m"));
        assert!(!output.contains("\x1b[0m\x1b[48;5;22m\x1b[38;5;"));
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

    /// 增删行分层：行号压暗、标记取增删色、正文回到默认前景。
    #[test]
    fn changed_line_numbers_are_dimmed_within_the_row_background() {
        let palette = DiffPalette::default();
        let added = style_added_line(Path::new("main.rs"), "  7 +  let total = 42;");

        assert!(added.contains(&format!(
            "\x1b[38;5;{FILL_LINE_NUMBER}m  7 {}+  \x1b[39m",
            palette.add_foreground.fg()
        )));
    }

    /// 不符合行号前缀结构的行整体交给语法高亮，不做拆分。
    #[test]
    fn lines_without_a_number_prefix_are_highlighted_as_a_whole() {
        let output = style_context_line(Path::new("main.rs"), "@@ hunk header");

        assert!(!output.contains(&format!("\x1b[38;5;{LINE_NUMBER_COLOR}m")));
    }

    /// 【差异配色测试】【浅色终端自适应】验证白底终端使用柔和浅绿/浅红底色与可读深色代码字。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn light_background_uses_soft_pastel_rows_and_dark_readable_text() {
        let light = DiffPalette::for_background((248, 249, 250), true);
        assert!(light.light);
        let added = style_added_line_with_palette(
            Path::new("main.rs"),
            "  3 +  let value = 42;",
            Some(4..9),
            light,
        );
        let removed = style_removed_line_with_palette(
            Path::new("main.rs"),
            "  3 -  let old = 42;",
            Some(4..7),
            light,
        );
        assert!(added.contains(&light.add_background.bg()));
        assert!(added.contains(&light.add_emphasis_background.bg()));
        assert!(!added.contains("\x1b[48;5;22m"));
        assert!(removed.contains(&light.delete_background.bg()));
        assert!(removed.contains(&light.delete_emphasis_background.bg()));
        assert!(!removed.contains("\x1b[48;5;52m"));
        assert!(
            crate::render::terminal_contrast::contrast(
                light.text_foreground,
                light.add_background.to_rgb()
            ) >= MIN_TEXT_CONTRAST
        );
        assert!(
            crate::render::terminal_contrast::contrast(
                light.text_foreground,
                light.delete_background.to_rgb()
            ) >= MIN_TEXT_CONTRAST
        );

        let light_256 = DiffPalette::for_background((250, 250, 250), false);
        assert_eq!(light_256.add_background, DiffColor::Ansi256(194));
        assert_eq!(light_256.delete_background, DiffColor::Ansi256(224));
    }
}
