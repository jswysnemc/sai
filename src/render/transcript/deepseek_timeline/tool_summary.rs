use crate::render::terminal_text as t;
use crate::render::tool_view::ToolView;
use serde_json::Value;
use unicode_width::UnicodeWidthStr;

/// 【终端】【常用工具摘要】为写入、替换、全局搜索与命令等常用工具提取关键语义标签。
///
/// 参数:
/// - `view`: 工具调用视图
/// - `elapsed`: 耗时字符串
/// - `width`: 允许的最大宽度
/// - `approval`: 是否等待审批
///
/// 返回:
/// - 结构化提取出的摘要文本，不支持的工具返回 None
pub(super) fn label(
    view: &ToolView,
    elapsed: &str,
    width: usize,
    approval: bool,
) -> Option<String> {
    match view.name.as_str() {
        "write_file" => summarize_write(view, elapsed, width, approval),
        "str_replace" => summarize_replace(view, elapsed, width, approval),
        "grep" => summarize_grep(view, elapsed, width),
        "glob" => summarize_glob(view, elapsed, width),
        _ => None,
    }
}

/// 【终端】【文件写入摘要】提取路径与写入行数，窄屏优先保证文件名完整。
fn summarize_write(
    view: &ToolView,
    elapsed: &str,
    width: usize,
    approval: bool,
) -> Option<String> {
    let args: Value = serde_json::from_str(&view.arguments).ok()?;
    let path = args.get("path")?.as_str()?;
    let lines_count = args
        .get("content")
        .and_then(Value::as_str)
        .map(|c| c.lines().count())
        .unwrap_or(0);

    let filename = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let line_hint = if lines_count > 0 {
        format!(" ({lines_count} L)")
    } else {
        String::new()
    };

    let action = if approval {
        t("Approve write", "待审批写入")
    } else if view.outcome.is_some() {
        t("Wrote", "已写入")
    } else {
        t("Writing", "写入中")
    };

    // 1. 【终端】【多级候选】尝试完整路径、短路径与纯文件名
    for candidate in [
        format!("{action}{elapsed} · {path}{line_hint}"),
        format!("{action} · {path}{line_hint}"),
        format!("{action} · …/{filename}{line_hint}"),
        format!("{action} · {filename}"),
    ] {
        if UnicodeWidthStr::width(candidate.as_str()) <= width {
            return Some(candidate);
        }
    }

    Some(crate::render::clip_to_width(
        &format!("{action} · {filename}"),
        width,
        "…",
    ))
}

/// 【终端】【文本替换摘要】提取被修改文件路径。
fn summarize_replace(
    view: &ToolView,
    elapsed: &str,
    width: usize,
    approval: bool,
) -> Option<String> {
    let args: Value = serde_json::from_str(&view.arguments).ok()?;
    let path = args.get("path")?.as_str()?;
    let filename = path.rsplit(['/', '\\']).next().unwrap_or(path);

    let action = if approval {
        t("Approve patch", "待审批修改")
    } else if view.outcome.is_some() {
        t("Patched", "已修改")
    } else {
        t("Patching", "修改中")
    };

    for candidate in [
        format!("{action}{elapsed} · {path}"),
        format!("{action} · {path}"),
        format!("{action} · …/{filename}"),
        format!("{action} · {filename}"),
    ] {
        if UnicodeWidthStr::width(candidate.as_str()) <= width {
            return Some(candidate);
        }
    }

    Some(crate::render::clip_to_width(
        &format!("{action} · {filename}"),
        width,
        "…",
    ))
}

/// 【终端】【内容检索摘要】提取 pattern 关键词。
fn summarize_grep(view: &ToolView, elapsed: &str, width: usize) -> Option<String> {
    let args: Value = serde_json::from_str(&view.arguments).ok()?;
    let pattern = args.get("pattern")?.as_str()?;
    let action = if view.outcome.is_some() {
        t("Found in files", "已检索文本")
    } else {
        t("Searching files", "检索文本中")
    };

    for candidate in [
        format!("{action}{elapsed} · \"{pattern}\""),
        format!("{action} · \"{pattern}\""),
        format!("Grep · \"{pattern}\""),
    ] {
        if UnicodeWidthStr::width(candidate.as_str()) <= width {
            return Some(candidate);
        }
    }

    Some(crate::render::clip_to_width(
        &format!("{action} · \"{pattern}\""),
        width,
        "…",
    ))
}

/// 【终端】【路径匹配摘要】提取 glob 过滤通配符。
fn summarize_glob(view: &ToolView, elapsed: &str, width: usize) -> Option<String> {
    let args: Value = serde_json::from_str(&view.arguments).ok()?;
    let pattern = args.get("pattern")?.as_str()?;
    let action = if view.outcome.is_some() {
        t("Matched paths", "已匹配路径")
    } else {
        t("Matching paths", "匹配路径中")
    };

    for candidate in [
        format!("{action}{elapsed} · {pattern}"),
        format!("{action} · {pattern}"),
        format!("Glob · {pattern}"),
    ] {
        if UnicodeWidthStr::width(candidate.as_str()) <= width {
            return Some(candidate);
        }
    }

    Some(crate::render::clip_to_width(
        &format!("{action} · {pattern}"),
        width,
        "…",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_summarize_write_file() {
        let mut view = ToolView::running(
            "write_file".into(),
            serde_json::json!({
                "path": "src/main.rs",
                "content": "line1\nline2\nline3\n"
            })
            .to_string(),
        );
        view.finish(true, String::new());
        let label = label(&view, " · 10ms", 80, false).unwrap();
        assert!(label.contains("Wrote") || label.contains("已写入"));
        assert!(label.contains("main.rs"));
    }
}
