use super::line::AnsiLine;
use crate::render::brand_logo::{logo_lines, LOGO_HEIGHT, LOGO_WIDTH};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// 品牌标志使用的实心块样式（与 Web 端 --signal 同色）。
const LOGO_STYLE: &str = "\x1b[38;2;58;114;100m";
/// 标志与右侧信息列之间的间隔列数。
const LOGO_CONTENT_GAP: usize = 2;
/// 面板内部左右各保留的留白列数。
const PANEL_SIDE_PADDING: usize = 1;
/// 信息列的最小可用宽度，低于该宽度就放弃标志改为纯信息面板。
const MIN_CONTENT_WIDTH: usize = 28;
/// 边框样式
const BORDER_STYLE: &str = "\x1b[2m";
/// 字段标签样式
const LABEL_STYLE: &str = "\x1b[2m";
/// 提示文本样式
const HINT_STYLE: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

/// REPL 启动时显示的会话基础信息。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WelcomeCell {
    pub(crate) version: String,
    pub(crate) model: String,
    pub(crate) directory: String,
    pub(crate) permissions: String,
}

/// 按当前终端宽度渲染启动面板。
///
/// 品牌标志置于边框内部左侧，信息列在其右侧；终端过窄时省略标志，
/// 优先保证会话信息完整。
///
/// 参数:
/// - `cell`: 启动信息 source
/// - `width`: 终端列数
///
/// 返回:
/// - 不需要再次换行的 ANSI 行
pub(crate) fn display_lines(cell: &WelcomeCell, width: usize) -> Vec<AnsiLine> {
    // 1. 先按可用宽度决定是否容得下框内标志
    let inner_width = width.saturating_sub(2).clamp(24, 76);
    let logo_reserved = LOGO_WIDTH + LOGO_CONTENT_GAP;
    let content_width = inner_width
        .saturating_sub(PANEL_SIDE_PADDING * 2)
        .saturating_sub(logo_reserved);
    let show_logo = content_width >= MIN_CONTENT_WIDTH;

    // 2. 生成信息列：标志占位后剩余宽度即为其可用宽度
    let content_width = if show_logo {
        content_width
    } else {
        inner_width.saturating_sub(PANEL_SIDE_PADDING * 2)
    };
    let rows = content_rows(cell, content_width);

    // 3. 逐行拼装边框、标志与信息列
    let mut lines = vec![AnsiLine::new(top_border(&cell.version, inner_width))];
    let logo = if show_logo {
        logo_lines(LOGO_STYLE)
    } else {
        Vec::new()
    };
    let blank_logo = " ".repeat(LOGO_WIDTH);
    let body_rows = rows.len().max(if show_logo { LOGO_HEIGHT } else { 0 });
    for index in 0..body_rows {
        let logo_cell = if show_logo {
            let row = logo.get(index).cloned().unwrap_or_else(|| blank_logo.clone());
            format!("{row}{}", " ".repeat(LOGO_CONTENT_GAP))
        } else {
            String::new()
        };
        let content = rows.get(index).cloned().unwrap_or_default();
        lines.push(AnsiLine::new(body_line(&logo_cell, &content, inner_width)));
    }
    lines.push(AnsiLine::new(bottom_border(inner_width)));
    lines
}

/// 构造带标题的顶部边框。
///
/// 参数:
/// - `version`: 当前版本号
/// - `inner_width`: 面板内部宽度
///
/// 返回:
/// - 顶部边框行
fn top_border(version: &str, inner_width: usize) -> String {
    let title = format!("Sai (v{version})");
    let title = truncate_to_width(&title, inner_width.saturating_sub(4));
    let padding = inner_width.saturating_sub(UnicodeWidthStr::width(title.as_str()) + 3);
    format!(
        "{BORDER_STYLE}╭─{RESET} \x1b[1m{title}{RESET} {BORDER_STYLE}{}╮{RESET}",
        "─".repeat(padding)
    )
}

/// 构造底部边框。
///
/// 参数:
/// - `inner_width`: 面板内部宽度
///
/// 返回:
/// - 底部边框行
fn bottom_border(inner_width: usize) -> String {
    format!("{BORDER_STYLE}╰{}╯{RESET}", "─".repeat(inner_width))
}

/// 拼装一行面板正文。
///
/// 参数:
/// - `logo_cell`: 已含右侧间隔的标志片段，无标志时为空
/// - `content`: 信息列内容
/// - `inner_width`: 面板内部宽度
///
/// 返回:
/// - 含左右边框且宽度对齐的正文行
fn body_line(logo_cell: &str, content: &str, inner_width: usize) -> String {
    let padding = " ".repeat(PANEL_SIDE_PADDING);
    let body = format!("{padding}{logo_cell}{content}");
    let trailing = inner_width.saturating_sub(visible_width(&body));
    format!(
        "{BORDER_STYLE}│{RESET}{body}{}{BORDER_STYLE}│{RESET}",
        " ".repeat(trailing)
    )
}

/// 生成信息列的每一行。
///
/// 行数与标志高度对齐：首行留空作为视觉呼吸，随后依次是模型、目录与权限。
///
/// 参数:
/// - `cell`: 启动信息 source
/// - `width`: 信息列可用宽度
///
/// 返回:
/// - 已截断到可用宽度的信息行
fn content_rows(cell: &WelcomeCell, width: usize) -> Vec<String> {
    vec![
        String::new(),
        field_row("model:", &cell.model, Some("/model to change"), width),
        field_row("directory:", &cell.directory, None, width),
        permission_row(&cell.permissions, width),
        String::new(),
    ]
}

/// 渲染一行普通字段。
///
/// 参数:
/// - `label`: 字段标签
/// - `value`: 字段值
/// - `hint`: 可选提示
/// - `width`: 可用宽度
///
/// 返回:
/// - 标签弱化、值正常的信息行
fn field_row(label: &str, value: &str, hint: Option<&str>, width: usize) -> String {
    let label_width = UnicodeWidthStr::width(label);
    let hint_width = hint.map(UnicodeWidthStr::width).unwrap_or(0);
    let value_width = width
        .saturating_sub(label_width + 1)
        .saturating_sub(if hint_width > 0 { hint_width + 2 } else { 0 });
    let value = truncate_to_width(value, value_width);
    match hint {
        Some(hint) if hint_width > 0 => {
            format!("{LABEL_STYLE}{label}{RESET} {value}  {HINT_STYLE}{hint}{RESET}")
        }
        _ => format!("{LABEL_STYLE}{label}{RESET} {value}"),
    }
}

/// 渲染权限模式行。
///
/// 权限模式直接影响外部操作的放行范围，因此按风险着色：完全放行用警示色，
/// 需要确认的模式用中性强调色。
///
/// 参数:
/// - `permissions`: 权限模式描述
/// - `width`: 可用宽度
///
/// 返回:
/// - 已着色的权限行
fn permission_row(permissions: &str, width: usize) -> String {
    let label = "permissions:";
    let value_width = width.saturating_sub(UnicodeWidthStr::width(label) + 1);
    let value = truncate_to_width(permissions, value_width);
    let style = permission_style(&value);
    format!("{LABEL_STYLE}{label}{RESET} {style}{value}{RESET}")
}

/// 按权限模式选择着色。
///
/// 参数:
/// - `permissions`: 权限模式描述
///
/// 返回:
/// - ANSI 样式前缀
fn permission_style(permissions: &str) -> &'static str {
    let lowered = permissions.to_ascii_lowercase();
    if lowered.contains("yolo") {
        // 完全放行：橙色警示
        "\x1b[38;5;208m"
    } else if lowered.contains("plan") {
        // 只读规划：蓝色
        "\x1b[38;5;75m"
    } else {
        // 需要确认或自动审核：绿色
        "\x1b[38;5;108m"
    }
}

/// 将文本截断到指定显示宽度。
///
/// 参数:
/// - `value`: 原始文本
/// - `width`: 最大显示宽度
///
/// 返回:
/// - 不超过最大宽度的文本
fn truncate_to_width(value: &str, width: usize) -> String {
    if UnicodeWidthStr::width(value) <= width {
        return value.to_string();
    }
    if width <= 3 {
        return ".".repeat(width);
    }
    let mut output = String::new();
    let mut used = 0usize;
    for ch in value.chars() {
        let char_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used.saturating_add(char_width) > width - 3 {
            break;
        }
        output.push(ch);
        used = used.saturating_add(char_width);
    }
    output.push_str("...");
    output
}

/// 计算含 ANSI 样式文本的显示宽度。
///
/// 参数:
/// - `value`: 带 ANSI 样式的文本
///
/// 返回:
/// - 可见字符的显示宽度
fn visible_width(value: &str) -> usize {
    let mut width = 0usize;
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if ('@'..='~').contains(&next) {
                    break;
                }
            }
            continue;
        }
        width = width.saturating_add(UnicodeWidthChar::width(ch).unwrap_or(0));
    }
    width
}

#[cfg(test)]
mod tests {
    use super::{display_lines, permission_style, visible_width, WelcomeCell};

    /// 构造样例启动信息。
    ///
    /// 参数:
    /// - `permissions`: 权限模式描述
    ///
    /// 返回:
    /// - 启动信息 source
    fn sample(permissions: &str) -> WelcomeCell {
        WelcomeCell {
            version: "0.1.4".to_string(),
            model: "gpt-5".to_string(),
            directory: "/workspace".to_string(),
            permissions: permissions.to_string(),
        }
    }

    /// 验证启动面板包含会话关键字段。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn welcome_panel_contains_runtime_details() {
        let lines = display_lines(&sample("YOLO mode"), 80);
        let joined = lines
            .iter()
            .map(|line| line.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(joined.contains("gpt-5"));
        assert!(joined.contains("permissions:"));
        assert!(joined.contains("Sai (v0.1.4)"));
    }

    /// 【终端】【品牌标志】验证标志渲染在边框内部且每行宽度一致。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn logo_renders_inside_the_border() {
        let lines = display_lines(&sample("YOLO mode"), 80);

        // 标志块必须出现在带边框的正文行内，而不是边框之外
        let logo_rows = lines
            .iter()
            .filter(|line| line.as_str().contains('█'))
            .collect::<Vec<_>>();
        assert!(!logo_rows.is_empty(), "宽终端应渲染标志");
        for line in &logo_rows {
            let text = line.as_str();
            assert!(text.contains('│'), "标志行必须位于边框内: {text}");
        }

        // 所有行等宽，否则右边框参差不齐
        let widths = lines
            .iter()
            .map(|line| visible_width(line.as_str()))
            .collect::<Vec<_>>();
        assert!(
            widths.windows(2).all(|pair| pair[0] == pair[1]),
            "面板每行宽度必须一致: {widths:?}"
        );
    }

    /// 【终端】【启动面板】验证窄终端省略标志但保留信息。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn narrow_terminal_drops_the_logo() {
        let lines = display_lines(&sample("YOLO mode"), 40);
        let joined = lines
            .iter()
            .map(|line| line.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(!joined.contains('█'), "窄终端不应渲染标志");
        assert!(joined.contains("gpt-5"));
    }

    /// 【终端】【启动面板】验证权限模式按风险着色且各不相同。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn permission_modes_use_distinct_colors() {
        let yolo = permission_style("YOLO mode");
        let plan = permission_style("Plan mode");
        let audited = permission_style("Confirm changes");

        assert_ne!(yolo, plan);
        assert_ne!(yolo, audited);
        assert_ne!(plan, audited);
        // YOLO 放行范围最大，必须用警示色
        assert!(yolo.contains("208"));
    }
}
