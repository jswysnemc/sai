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
        "write_file" | "str_replace" => summarize_edit(view, elapsed, width, approval),
        "grep" => summarize_grep(view, elapsed, width),
        "glob" => summarize_glob(view, elapsed, width),
        _ => None,
    }
}

/// 【终端】【编辑摘要】优先保留文件名与真实增删统计，路径和耗时按宽度逐级省略。
/// 参数: 工具视图、耗时、净宽和审批状态；返回: 带统计的编辑标题。
fn summarize_edit(view: &ToolView, elapsed: &str, width: usize, approval: bool) -> Option<String> {
    let args: Value = serde_json::from_str(&view.arguments).ok()?;
    let path = crate::render::ansi_clip::strip_ansi(args.get("path")?.as_str()?)
        .replace(['\n', '\r', '\t'], " ");
    let filename = path.rsplit(['/', '\\']).next().unwrap_or(&path);
    let failed = view.outcome.as_ref().is_some_and(|outcome| !outcome.ok);
    let action = if failed {
        t("Edit failed", "编辑失败")
    } else if approval {
        t("Approve edit", "待审批编辑")
    } else if view.name == "write_file" {
        if view.outcome.is_some() {
            t("Wrote", "已写入")
        } else {
            t("Writing", "写入中")
        }
    } else if view.outcome.is_some() {
        t("Replaced", "已替换")
    } else {
        t("Replacing", "替换中")
    };
    let stats = crate::render::tool_view::edit_stat_status(view)
        .map(|stats| format!(" · {}", crate::render::ansi_clip::strip_ansi(&stats)))
        .unwrap_or_default();
    for candidate in [
        format!("{action}{elapsed} · {path}{stats}"),
        format!("{action} · {path}{stats}"),
        format!("{action} · …/{filename}{stats}"),
        format!("{action} · {filename}{stats}"),
    ] {
        if UnicodeWidthStr::width(candidate.as_str()) <= width {
            return Some(candidate);
        }
    }
    let prefix = format!("{action} · ");
    let budget = width.saturating_sub(
        UnicodeWidthStr::width(prefix.as_str()) + UnicodeWidthStr::width(stats.as_str()),
    );
    let name = crate::render::clip_to_width(filename, budget, "…");
    Some(crate::render::clip_to_width(
        &format!("{prefix}{name}{stats}"),
        width,
        "",
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
