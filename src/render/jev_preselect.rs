use super::terminal_text;

/// 把发送前的 Jev 判断收成一行终端提示。
///
/// running 阶段不落行：TUI 不能替换已写出的提示，等判断结束再显示结论。
///
/// 参数:
/// - `phase`: running、ready、empty 或 failed
/// - `detail`: ready 时的暴露 JSON，或 failed 时的错误摘要
///
/// 返回:
/// - 不含 ANSI 的单行文本；判断尚未结束时为空
pub(crate) fn format_jev_preselect(phase: &str, detail: &str) -> Option<String> {
    let body = match phase {
        "running" => return None,
        "empty" => terminal_text("no new tools or skills", "没有新的工具或 Skill").to_string(),
        "failed" => {
            let summary = terminal_text(
                "judgment failed; base tools only",
                "判断失败，本轮只用基础工具",
            );
            let extra = detail.trim();
            if extra.is_empty() {
                summary.to_string()
            } else {
                format!("{summary}: {extra}")
            }
        }
        _ => jev_exposure_names(detail).unwrap_or_else(|| {
            terminal_text("no new tools or skills", "没有新的工具或 Skill").to_string()
        }),
    };
    Some(format!(
        "{} · {body}",
        terminal_text("Jev before send", "Jev · 发送前")
    ))
}

/// 从预选 JSON 取出工具名和 skill 名。
///
/// 参数:
/// - `detail`: announce_selection 返回的 JSON
///
/// 返回:
/// - 逗号分隔的名单；解析失败或名单为空时为空
fn jev_exposure_names(detail: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(detail).ok()?;
    if value.get("ok") != Some(&serde_json::Value::Bool(true)) {
        return None;
    }
    let mut names = Vec::new();
    for item in value
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        if let Some(name) = item.get("name").and_then(serde_json::Value::as_str) {
            let name = name.trim();
            if !name.is_empty() {
                names.push(name.to_string());
            }
        }
    }
    for item in value
        .get("skills")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        if let Some(name) = item.get("name").and_then(serde_json::Value::as_str) {
            let name = name.trim();
            if !name.is_empty() {
                names.push(format!("skill:{name}"));
            }
        }
    }
    if names.is_empty() {
        None
    } else {
        Some(names.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::format_jev_preselect;

    #[test]
    fn ready_lists_tools_and_skills() {
        let detail = r#"{"ok":true,"router":"jev","tools":[{"name":"web_search"}],"skills":[{"name":"drawio"}]}"#;
        assert_eq!(
            format_jev_preselect("ready", detail).as_deref(),
            Some("Jev before send · web_search, skill:drawio")
        );
    }

    #[test]
    fn running_stays_silent_until_the_judgment_finishes() {
        assert_eq!(format_jev_preselect("running", ""), None);
    }

    #[test]
    fn empty_and_failed_stay_visible() {
        assert_eq!(
            format_jev_preselect("empty", "").as_deref(),
            Some("Jev before send · no new tools or skills")
        );
        assert_eq!(
            format_jev_preselect("failed", "timeout").as_deref(),
            Some("Jev before send · judgment failed; base tools only: timeout")
        );
    }
}
