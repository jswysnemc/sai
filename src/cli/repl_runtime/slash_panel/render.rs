//! 斜杠命令面板行渲染：无实心背景衬块，按色板语义色突出当前命令、分类标签与禁用原因。

use crate::cli::repl_commands::{command_category, ReplCommandSuggestion};
use crate::cli::repl_text::visible_width;
use crate::i18n::text as t;
use crate::render::ansi_clip::ansi_visible_width;
use crate::render::panel_palette::{fg, PanelPalette};

const RESET: &str = "\x1b[0m";

/// 【终端】【命令面板顶栏】渲染面板标题、可见范围计数值与操作引导细线。
/// 参数: `start` 为窗口首项下标，`visible` 为可见命令数，`total` 为总数，`cols` 为终端列数；返回: 顶栏文本行。
pub(super) fn header_line(start: usize, visible: usize, total: usize, cols: usize) -> String {
    let palette = PanelPalette::current();
    let up = if start > 0 { "▲" } else { "" };
    let down = if start + visible < total { "▼" } else { "" };
    let arrows = format!("{up}{down}");
    let range = if arrows.is_empty() {
        format!("{}-{}/{}", start + 1, start + visible, total)
    } else {
        format!("{}-{}/{} {arrows}", start + 1, start + visible, total)
    };
    let title = format!(
        " {}◆{} \x1b[1m{}{}{RESET} {}· {range}{RESET}",
        fg(palette.accent),
        RESET,
        fg(palette.text),
        t("Commands", "斜杠命令"),
        fg(palette.muted),
    );
    let hint = format!(
        "{}{}{RESET}",
        fg(palette.muted),
        t(
            "↑↓ navigate · Tab complete · Esc close",
            "↑↓ 选择 · Tab 补全 · Esc 关闭"
        ),
    );
    let title_w = ansi_visible_width(&title);
    let hint_w = ansi_visible_width(&hint);
    if cols >= title_w + hint_w + 5 {
        let gap = cols - title_w - hint_w - 4;
        return format!(
            "{title} {}{}{} {hint} ",
            fg(palette.border),
            "─".repeat(gap),
            RESET,
        );
    }
    let fill = cols.saturating_sub(title_w + 2);
    format!("{title} {}{}{RESET}", fg(palette.border), "─".repeat(fill))
}

/// 【终端】【命令条目】按选中态、分类与禁用状态格式化单条斜杠命令，对齐输入框第 3 列基线。
/// 参数: `suggestion` 为命令条目，`cmd_width` 为命令列宽，`cols` 为总列数，`selected` 为是否高亮；返回: ANSI 建议行。
pub(super) fn format_suggestion(
    suggestion: ReplCommandSuggestion,
    cmd_width: usize,
    cols: usize,
    selected: bool,
) -> String {
    let palette = PanelPalette::current();
    let command_width = cmd_width.clamp(12, 22).min(cols.saturating_sub(4));
    let marker = if suggestion.disabled {
        "·"
    } else if selected {
        "→"
    } else {
        " "
    };
    let category = command_category(suggestion.command);
    let show_cat = !category.is_empty() && cols >= command_width + 25;
    let cat_width = if show_cat { 9usize } else { 0usize };
    let used_prefix = 3 + command_width + cat_width;
    let description_width = cols.saturating_sub(used_prefix);

    // 1. 【终端】【禁用提示】运行中不可用的命令始终整体灰化，并切换说明告知何时可执行
    let description = if suggestion.disabled {
        truncate_to_width(
            t("available after this turn", "本轮结束后可用"),
            description_width,
        )
    } else {
        truncate_to_width(suggestion.description, description_width)
    };
    let cmd_padded = pad_right(
        &truncate_to_width(suggestion.command, command_width),
        command_width,
    );

    if suggestion.disabled {
        let cat_part = if show_cat {
            pad_right(category, cat_width)
        } else {
            String::new()
        };
        return format!(" {marker} \x1b[90m{cmd_padded}{cat_part}{description}{RESET}");
    }

    if selected {
        // 2. 【终端】【选中样式】使用色板自适应正文色加粗替代硬编码亮白，保证浅色终端清晰可读
        let cat_part = if show_cat {
            format!(
                "{}{}{RESET}",
                fg(palette.added),
                pad_right(category, cat_width),
            )
        } else {
            String::new()
        };
        return format!(
            " {}{marker}{RESET} \x1b[1m{}{cmd_padded}{RESET}{cat_part}{}{description}{RESET}",
            fg(palette.accent),
            fg(palette.text),
            fg(palette.text),
        );
    }

    // 3. 【终端】【默认样式】非选中项保持第 3 列基线对齐与柔和前景
    let cat_part = if show_cat {
        format!(
            "\x1b[2m{}{}{RESET}",
            fg(palette.subtle),
            pad_right(category, cat_width),
        )
    } else {
        String::new()
    };
    format!(
        " {marker} \x1b[2m{}{cmd_padded}{RESET}{cat_part}\x1b[2m{}{description}{RESET}",
        fg(palette.muted),
        fg(palette.muted),
    )
}

/// 【终端】【右侧补齐】按显示宽度补足空格，确保多语言分类列竖直对齐。
/// 参数: `value` 为原始文本，`target` 为目标显示宽度；返回: 至少占 `target` 列的文本。
fn pad_right(value: &str, target: usize) -> String {
    let width = visible_width(value);
    if width >= target {
        value.to_string()
    } else {
        format!("{value}{}", " ".repeat(target - width))
    }
}

/// 【终端】【宽度裁剪】将文本裁剪至指定显示宽度，超长时附加单格省略号。
/// 参数: `value` 为原始文本，`width` 为最大显示列宽；返回: 适配宽度后的文本。
pub(super) fn truncate_to_width(value: &str, width: usize) -> String {
    if visible_width(value) <= width {
        return value.to_string();
    }
    if width == 0 {
        return String::new();
    }
    let mut output = String::new();
    let mut used = 0usize;
    let limit = width.saturating_sub(1);
    for ch in value.chars() {
        let char_width = visible_width(&ch.to_string());
        if used.saturating_add(char_width) > limit {
            break;
        }
        output.push(ch);
        used = used.saturating_add(char_width);
    }
    output.push('…');
    output
}
