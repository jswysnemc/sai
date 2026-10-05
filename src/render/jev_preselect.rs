use super::terminal_text;

/// 行内展示提示词片段说明的最大字符数。
const PROMPT_LABEL_CHARS: usize = 24;

/// 把发送前的 Jev 判断收成一行终端提示。
///
/// running 阶段不落行：TUI 不能替换已写出的提示，等判断结束再显示结论。
/// 文案按「主语 + 动作 + 分类名单」组织：`Jev picked tools: a, b · skills: c`。
///
/// 参数:
/// - `phase`: running、ready、empty 或 failed
/// - `detail`: ready 时的暴露 JSON，或 failed 时的错误摘要
///
/// 返回:
/// - 不含 ANSI 的单行文本；判断尚未结束时为空
pub(crate) fn format_jev_preselect(phase: &str, detail: &str) -> Option<String> {
    let nothing = terminal_text(
        "Jev picked nothing new for this message",
        "Jev 没有为本轮补充工具或上下文",
    );
    let line = match phase {
        "running" => return None,
        "empty" => nothing.to_string(),
        "failed" => {
            let summary = terminal_text(
                "Jev could not pick tools; using base tools only",
                "Jev 判断失败，本轮只用基础工具",
            );
            let extra = detail.trim();
            if extra.is_empty() {
                summary.to_string()
            } else {
                format!("{summary}: {extra}")
            }
        }
        _ => jev_exposure_groups(detail)
            .map(|groups| format!("{} {groups}", terminal_text("Jev picked", "Jev 选用了")))
            .unwrap_or_else(|| nothing.to_string()),
    };
    Some(line)
}

/// 从预选 JSON 按类别整理工具、Skill、提示词片段与记忆。
///
/// 参数:
/// - `detail`: 预选结果 JSON（tools、skills、contexts）
///
/// 返回:
/// - 如 `tools: web_search, browser · skills: drawio · memory`；解析失败或全空时为空
fn jev_exposure_groups(detail: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(detail).ok()?;
    if value.get("ok") != Some(&serde_json::Value::Bool(true)) {
        return None;
    }
    // 1. 工具与 Skill 只取名称
    let names_of = |key: &str| {
        value
            .get(key)
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|item| item.get("name").and_then(serde_json::Value::as_str))
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>()
    };
    let tools = names_of("tools");
    let skills = names_of("skills");
    // 2. 注入片段：提示词片段显示说明，记忆只显示一个标记
    let mut prompts = Vec::new();
    let mut memory = false;
    for item in value
        .get("contexts")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        match item.get("kind").and_then(serde_json::Value::as_str) {
            Some("memory") => memory = true,
            Some("prompt") => {
                let description = item
                    .get("description")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                prompts.push(prompt_label(description));
            }
            _ => {}
        }
    }
    // 3. 按类别拼接，空类别省略
    let mut groups = Vec::new();
    for (label, items) in [
        (terminal_text("tools", "工具"), tools),
        (terminal_text("skills", "Skill"), skills),
        (terminal_text("prompts", "片段"), prompts),
    ] {
        if !items.is_empty() {
            groups.push(format!("{label}: {}", items.join(", ")));
        }
    }
    if memory {
        groups.push(terminal_text("memory", "记忆").to_string());
    }
    (!groups.is_empty()).then(|| groups.join(" · "))
}

/// 提示词片段的行内标签：说明折叠空白并截断，没有说明时只显示种类。
///
/// 参数:
/// - `description`: 片段说明
///
/// 返回:
/// - 截断后的说明；没有说明时为 `untitled`
fn prompt_label(description: &str) -> String {
    let collapsed = description.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return terminal_text("untitled", "未命名").to_string();
    }
    let mut label = collapsed
        .chars()
        .take(PROMPT_LABEL_CHARS)
        .collect::<String>();
    if collapsed.chars().count() > PROMPT_LABEL_CHARS {
        label.push('…');
    }
    label
}

#[cfg(test)]
mod tests {
    use super::format_jev_preselect;

    #[test]
    fn ready_lists_tools_and_skills() {
        let detail = r#"{"ok":true,"router":"jev","tools":[{"name":"web_search"}],"skills":[{"name":"drawio"}]}"#;
        assert_eq!(
            format_jev_preselect("ready", detail).as_deref(),
            Some("Jev picked tools: web_search · skills: drawio")
        );
    }

    /// 只命中片段与记忆时不再误报“没有新的工具”。
    #[test]
    fn ready_lists_prompt_segments_and_memory() {
        let detail = r#"{"ok":true,"router":"jev","tools":[],"skills":[],"contexts":[{"kind":"prompt","description":"编写或修改数据库迁移时使用的规范"},{"kind":"memory","description":"Memory index"}]}"#;
        assert_eq!(
            format_jev_preselect("ready", detail).as_deref(),
            Some("Jev picked prompts: 编写或修改数据库迁移时使用的规范 · memory")
        );
    }

    /// 工具、Skill 与片段同时命中时全部列出。
    #[test]
    fn ready_lists_every_kind_together() {
        let detail = r#"{"ok":true,"router":"jev","tools":[{"name":"web_search"}],"skills":[{"name":"drawio"}],"contexts":[{"kind":"prompt","description":"一段超过二十四个字符上限的非常长的片段说明文字需要截断"}]}"#;
        let line = format_jev_preselect("ready", detail).unwrap();
        assert!(
            line.starts_with("Jev picked tools: web_search · skills: drawio · prompts: "),
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
            Some("Jev picked nothing new for this message")
        );
        assert_eq!(
            format_jev_preselect("failed", "timeout").as_deref(),
            Some("Jev could not pick tools; using base tools only: timeout")
        );
    }
}
