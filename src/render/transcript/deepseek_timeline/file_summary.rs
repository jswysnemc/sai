use crate::render::terminal_text as t;
use crate::render::tool_view::{parse_read_page, ToolView};
use serde_json::Value;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// 【终端】【文件摘要】保留文件名和实际行范围，空间不足时依次省略目录、耗时与较长动作。
/// 参数: `view` 为工具，`elapsed` 为耗时，`width` 为净宽，`approval` 为审批状态；返回: 摘要或空值。
pub(super) fn label(
    view: &ToolView,
    elapsed: &str,
    width: usize,
    approval: bool,
) -> Option<String> {
    if view.name != "read_file" {
        return None;
    }
    if let Some(path) = crate::render::tool_view::image_read_path(view) {
        let path = crate::render::activity_animation::strip_ansi_for_test(&path)
            .replace(['\n', '\r', '\t'], " ");
        return Some(fit(
            &path,
            "",
            t("Viewed image", "已查看图片"),
            t("Image", "图片"),
            elapsed,
            width,
        ));
    }
    let arguments: Value = serde_json::from_str(&view.arguments).ok()?;
    let requested_path = arguments.get("path")?.as_str()?;
    let page = view
        .outcome
        .as_ref()
        .filter(|outcome| outcome.ok)
        .and_then(|outcome| parse_read_page(&outcome.output));
    let path = page
        .as_ref()
        .map(|page| page.path.as_str())
        .filter(|path| !path.is_empty())
        .unwrap_or(requested_path);
    let path = crate::render::activity_animation::strip_ansi_for_test(path)
        .replace(['\n', '\r', '\t'], " ");
    let range = if let Some(page) = page {
        line_range(page.offset as u64, page.content.len() as u64)
    } else if view.outcome.is_none() {
        line_range(
            arguments
                .get("offset")
                .and_then(Value::as_u64)
                .unwrap_or(1)
                .max(1),
            arguments.get("limit").and_then(Value::as_u64).unwrap_or(0),
        )
    } else {
        String::new()
    };
    let failed = view.outcome.as_ref().is_some_and(|outcome| !outcome.ok);
    let (action, short) = if failed {
        (t("Read failed", "读取失败"), t("Failed", "失败"))
    } else if approval {
        (
            t("Awaiting approval", "等待审批"),
            t("Approval pending", "待审批"),
        )
    } else if view.outcome.is_some() {
        (t("Read file", "读取文件"), t("Read", "读取"))
    } else {
        (t("Reading file", "读取文件"), t("Reading", "读取"))
    };
    Some(fit(&path, &range, action, short, elapsed, width))
}

/// 【终端】【文件范围】单行使用单个行号，空页不显示范围。
/// 参数: `start` 为起始行，`count` 为返回或请求行数；返回: 带括号的行范围。
fn line_range(start: u64, count: u64) -> String {
    match count {
        0 => String::new(),
        1 => format!(" (L{start})"),
        _ => format!(" (L{start}–{})", start.saturating_add(count - 1)),
    }
}

/// 【终端】【文件裁剪】先缩短目录，必要时移除耗时，最后裁剪文件名中段而保留扩展名。
/// 参数: `path` 为路径，`range` 为行范围，`action` 和 `short` 为动作，`elapsed` 为耗时，`width` 为净宽。
/// 返回: 不超过显示列数的摘要。
fn fit(path: &str, range: &str, action: &str, short: &str, elapsed: &str, width: usize) -> String {
    let filename = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let abbreviated = if filename == path {
        filename.to_string()
    } else {
        format!("…/{filename}")
    };
    // 1. 【终端】【文件裁剪】完整路径优先，宽度不足时仍保证文件名与行范围完整
    for prefix in [
        format!("{action}{elapsed} · "),
        format!("{action} · "),
        format!("{short} · "),
    ] {
        for candidate in [path, abbreviated.as_str(), filename] {
            let label = format!("{prefix}{candidate}{range}");
            if UnicodeWidthStr::width(label.as_str()) <= width {
                return label;
            }
        }
    }
    // 2. 【终端】【文件裁剪】超长文件名保留首尾，极窄终端最后按整行预算裁剪
    let prefix = format!("{short} · ");
    let fixed_width = UnicodeWidthStr::width(prefix.as_str()) + UnicodeWidthStr::width(range);
    let filename = clip_middle(filename, width.saturating_sub(fixed_width));
    crate::render::clip_to_width(&format!("{prefix}{filename}{range}"), width, "")
}

/// 【终端】【文件裁剪】按字形和显示列数裁剪中段，避免中文或组合字符越界。
/// 参数: `text` 为文件名，`width` 为可用列数；返回: 保留首尾的文件名。
fn clip_middle(text: &str, width: usize) -> String {
    if UnicodeWidthStr::width(text) <= width {
        return text.to_string();
    }
    if width == 0 {
        return String::new();
    }
    let suffix_budget = width.saturating_sub(1) / 2;
    let mut suffix = Vec::new();
    let mut suffix_width = 0;
    for grapheme in text.graphemes(true).rev() {
        let next_width = UnicodeWidthStr::width(grapheme);
        if suffix_width + next_width > suffix_budget {
            break;
        }
        suffix.push(grapheme);
        suffix_width += next_width;
    }
    suffix.reverse();
    let prefix = crate::render::clip_to_width(text, width - suffix_width - 1, "");
    format!("{prefix}…{}", suffix.concat())
}
