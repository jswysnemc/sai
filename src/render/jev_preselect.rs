use super::terminal_text;

/// 行内展示提示词片段说明的最大字符数。
const PROMPT_LABEL_CHARS: usize = 24;

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
        "empty" => {
            terminal_text("nothing new selected", "没有新的工具、Skill 或上下文").to_string()
        }
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
            terminal_text("nothing new selected", "没有新的工具、Skill 或上下文").to_string()
        }),
    };
    Some(format!(
        "{} · {body}",
        terminal_text("Jev before send", "Jev · 发送前")
    ))
}

/// 从预选 JSON 取出工具名、skill 名、提示词片段说明与记忆标记。
///
/// 参数:
/// - `detail`: 预选结果 JSON（tools、skills、contexts）
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
    // 注入片段：提示词片段显示说明，记忆只显示一个标记
    for item in value
        .get("contexts")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        match item.get("kind").and_then(serde_json::Value::as_str) {
            Some("memory") => names.push(terminal_text("memory", "记忆").to_string()),
            Some("prompt") => {
                let description = item
                    .get("description")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                names.push(prompt_label(description));
            }
            _ => {}
        }
    }
    if names.is_empty() {
        None
    } else {
        Some(names.join(", "))
    }
}

/// 提示词片段的行内标签：说明折叠空白并截断，没有说明时只显示种类。
///
/// 参数:
/// - `description`: 片段说明
///
/// 返回:
/// - 形如 `prompt:编写迁移时使用` 的标签
fn prompt_label(description: &str) -> String {
    let prefix = terminal_text("prompt", "片段");
    let collapsed = description.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return prefix.to_string();
    }
    let mut label = collapsed
        .chars()
        .take(PROMPT_LABEL_CHARS)
        .collect::<String>();
    if collapsed.chars().count() > PROMPT_LABEL_CHARS {
        label.push('…');
    }
    format!("{prefix}:{label}")
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

    /// 只命中片段与记忆时不再误报“没有新的工具”。
    #[test]
    fn ready_lists_prompt_segments_and_memory() {
        let detail = r#"{"ok":true,"router":"jev","tools":[],"skills":[],"contexts":[{"kind":"prompt","description":"编写或修改数据库迁移时使用的规范"},{"kind":"memory","description":"Memory index"}]}"#;
        assert_eq!(
            format_jev_preselect("ready", detail).as_deref(),
            Some("Jev before send · prompt:编写或修改数据库迁移时使用的规范, memory")
        );
    }

    /// 工具、Skill 与片段同时命中时全部列出。
    #[test]
    fn ready_lists_every_kind_together() {
        let detail = r#"{"ok":true,"router":"jev","tools":[{"name":"web_search"}],"skills":[{"name":"drawio"}],"contexts":[{"kind":"prompt","description":"一段超过二十四个字符上限的非常长的片段说明文字需要截断"}]}"#;
        let line = format_jev_preselect("ready", detail).unwrap();
        assert!(
            line.starts_with("Jev before send · web_search, skill:drawio, prompt:"),
            "{line}"
        );
        assert!(line.ends_with('…'), "{line}");
    }

    #[test]
    fn running_stays_silent_until_the_judgment_finishes() {
        assert_eq!(format_jev_preselect("running", ""), None);
    }

    #[test]
    fn empty_and_failed_stay_visible() {
        assert_eq!(
            format_jev_preselect("empty", "").as_deref(),
            Some("Jev before send · nothing new selected")
        );
        assert_eq!(
            format_jev_preselect("failed", "timeout").as_deref(),
            Some("Jev before send · judgment failed; base tools only: timeout")
        );
    }
}
