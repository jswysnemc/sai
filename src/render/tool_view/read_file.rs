use crate::render::status_style::{color_status, ToolHealth};
use crate::render::tool_event_line::{tool_event_label_tense, tool_status_line, ToolVerbTense};
use crate::render::ToolCallDisplayMode;
use serde_json::Value;

/// read_file 文本分页结果的可渲染模型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReadPageView {
    /// 展示路径
    pub path: String,
    /// 本次读取起始行号
    pub offset: usize,
    /// 是否被截断（还有下一页）
    pub truncated: bool,
    /// 下一页起始行号
    pub next: Option<usize>,
    /// 已带 `N: ` 前缀的内容行
    pub content: Vec<String>,
}

/// 行号列前景色：与 diff 行号一致的 244 号中性灰。
const LINE_NUMBER_COLOR: u8 = 244;

/// 解析 read_file 的 text-page 结果 JSON。
///
/// 参数:
/// - `output`: 工具原始输出
///
/// 返回:
/// - 文本分页视图；不是 text-page 时返回空
pub(crate) fn parse_read_page(output: &str) -> Option<ReadPageView> {
    let value = serde_json::from_str::<Value>(output.trim()).ok()?;
    if value.get("type")?.as_str()? != "text-page" {
        return None;
    }
    let content = value
        .get("content")?
        .as_str()?
        .lines()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    Some(ReadPageView {
        path: value
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        offset: value.get("offset").and_then(Value::as_u64).unwrap_or(1) as usize,
        truncated: value
            .get("truncated")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        next: value
            .get("next")
            .and_then(Value::as_u64)
            .map(|item| item as usize),
        content,
    })
}

/// 渲染 read_file 定稿后的完整工具视图。
///
/// 状态行：`• Read path:offset+limit 10-19`；
/// 正文行号列用中性灰着色，与 diff 行号视觉同源。
///
/// 参数:
/// - `view`: read 工具生命周期
/// - `mode`: 工具展示模式
///
/// 返回:
/// - ANSI 工具视图文本
pub(crate) fn render(view: &super::model::ToolView, mode: ToolCallDisplayMode) -> Option<String> {
    if mode == ToolCallDisplayMode::Hidden {
        return Some(String::new());
    }
    let outcome = view.outcome.as_ref()?;
    let page = parse_read_page(&outcome.output)?;
    let label = tool_event_label_tense("read_file", Some(&view.arguments), ToolVerbTense::Perfect);
    let badge = read_page_badge(&page);
    let health = if outcome.ok {
        ToolHealth::Ok
    } else {
        ToolHealth::Err
    };
    let mut output = tool_status_line(&label, &badge, health);
    if !outcome.ok {
        return Some(output);
    }
    // Summary 只保留状态行；正文进入 Ctrl+O 阅读面板
    if mode == ToolCallDisplayMode::Summary {
        return Some(output);
    }
    for line in &page.content {
        output.push('\n');
        output.push_str(&render_content_line(line));
    }
    if page.truncated {
        if let Some(next) = page.next {
            output.push_str(&format!(
                "\n\x1b[2m\x1b[36m  └ … {} offset={next}\x1b[0m",
                crate::i18n::text("next page from line", "下一页从行")
            ));
        }
    }
    Some(output)
}

/// 生成状态行的阅读进度徽标。
///
/// 参数:
/// - `page`: 文本分页视图
///
/// 返回:
/// - `x-y` 区间徽标；空页返回 empty
fn read_page_badge(page: &ReadPageView) -> String {
    if page.content.is_empty() {
        return color_status("empty");
    }
    let first = page.offset;
    let last = first + page.content.len().saturating_sub(1);
    format!("\x1b[36m{first}-{last}\x1b[0m")
}

/// 渲染单条 `N: text` 内容行：行号列灰色，正文默认色。
///
/// 参数:
/// - `line`: 已带行号前缀的内容行
///
/// 返回:
/// - 着色后的内容行
fn render_content_line(line: &str) -> String {
    let Some((number, rest)) = line.split_once(": ") else {
        return format!("\x1b[2m    {line}\x1b[0m");
    };
    if !number.chars().all(|ch| ch.is_ascii_digit()) || number.is_empty() {
        return format!("\x1b[2m    {line}\x1b[0m");
    }
    format!("\x1b[2m\x1b[38;5;{LINE_NUMBER_COLOR}m{number}\x1b[0m {rest}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::activity_animation::strip_ansi_for_test;

    /// 行号列被识别并着色，正文保持原样。
    #[test]
    fn content_line_colors_the_number_column() {
        let rendered = render_content_line("42: let x = 1;");
        let plain = strip_ansi_for_test(&rendered);
        assert!(plain.contains("42"), "{plain}");
        assert!(plain.contains("let x = 1;"), "{plain}");
        assert!(rendered.contains("\x1b[38;5;244m"), "{rendered}");
    }

    /// 非标准行号格式按普通弱化行处理。
    #[test]
    fn non_numbered_line_falls_back_to_plain() {
        let rendered = render_content_line("no prefix here");
        assert!(!rendered.contains("\x1b[38;5;244m"));
    }

    /// 分页徽标输出首末行区间。
    #[test]
    fn badge_reports_the_read_range() {
        let page = ReadPageView {
            path: "a.rs".to_string(),
            offset: 10,
            truncated: true,
            next: Some(20),
            content: vec!["10: a".to_string(), "11: b".to_string()],
        };
        let plain = strip_ansi_for_test(&read_page_badge(&page));
        assert!(plain.contains("10-11"), "{plain}");
    }

    /// text-page JSON 正确解析为分页视图。
    #[test]
    fn parses_text_page_json() {
        let output = r#"{"type":"text-page","path":"a.rs","offset":5,"limit":2,"content":"5: x\n6: y","truncated":true,"next":7}"#;
        let page = parse_read_page(output).unwrap();
        assert_eq!(page.offset, 5);
        assert_eq!(page.next, Some(7));
        assert!(page.truncated);
        assert_eq!(page.content, vec!["5: x".to_string(), "6: y".to_string()]);
    }
}
