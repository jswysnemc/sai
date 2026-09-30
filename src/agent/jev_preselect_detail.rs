//! 【Jev路由】【预选结果】把本轮暴露的工具、Skill 与注入片段合成一份界面可读的 JSON。
//!
//! 注入给模型的仍是原有文本块；这里只生成 `JevPreselect` 事件的 detail，
//! 让 TUI 与 Web 能统一展示工具、Skill、提示词片段和记忆四类结果。

use super::jev_prompt_context::{Fragment, FragmentKind};
use serde_json::{json, Map, Value};

/// 界面预览正文的最大字符数；完整内容只注入给模型。
const PREVIEW_CHARS: usize = 1_200;

/// 合成预选事件的 detail。
///
/// 参数:
/// - `announced`: 工具与 Skill 暴露结果 JSON；本轮只命中片段时为空
/// - `fragments`: 本轮命中的提示词片段与记忆
///
/// 返回:
/// - `{"ok":true,"router":"jev","tools":[...],"skills":[...],"contexts":[...]}`
pub(super) fn preselect_detail<'a>(
    announced: Option<&str>,
    fragments: impl Iterator<Item = &'a Fragment>,
) -> String {
    // 1. 以暴露结果为底；解析失败或没有暴露时从空结果开始
    let mut record = announced
        .and_then(|text| serde_json::from_str::<Value>(text).ok())
        .and_then(|value| match value {
            Value::Object(map) => Some(map),
            _ => None,
        })
        .unwrap_or_else(|| {
            let mut map = Map::new();
            map.insert("ok".into(), Value::Bool(true));
            map.insert("router".into(), Value::String("jev".into()));
            map.insert("tools".into(), Value::Array(Vec::new()));
            map.insert("skills".into(), Value::Array(Vec::new()));
            map
        });
    // 2. 追加命中的片段与记忆，正文截断为预览
    let contexts = fragments.map(context_entry).collect::<Vec<_>>();
    record.insert("contexts".into(), Value::Array(contexts));
    Value::Object(record).to_string()
}

/// 单个注入片段的界面条目。
///
/// 参数:
/// - `fragment`: 命中的片段
///
/// 返回:
/// - 含种类、来源、说明与预览的 JSON 对象
fn context_entry(fragment: &Fragment) -> Value {
    let preview = match fragment.kind {
        // 记忆的有效信息是索引条目；使用契约是固定说明，不必重复展示
        FragmentKind::Memory => fragment
            .preview
            .as_deref()
            .map(memory_entries)
            .unwrap_or_default(),
        FragmentKind::Prompt => fragment.content.trim().to_string(),
    };
    json!({
        "id": fragment.id,
        "kind": fragment.kind.as_str(),
        "source": fragment.source,
        "description": fragment.description,
        "preview": truncate(&preview, PREVIEW_CHARS),
    })
}

/// 从记忆索引注入文本中取出条目行，去掉包裹标签与说明句。
///
/// 参数:
/// - `index`: 记忆索引注入文本
///
/// 返回:
/// - 分组标题与条目行
fn memory_entries(index: &str) -> String {
    index
        .lines()
        .map(str::trim)
        .filter(|line| {
            !line.is_empty()
                && !line.starts_with('<')
                && (line.starts_with('-') || line.ends_with('：') || line.ends_with(':'))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 按字符数截断并追加省略号。
///
/// 参数:
/// - `value`: 原文
/// - `max`: 最大字符数
///
/// 返回:
/// - 截断后的文本
fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    let mut output = value.chars().take(max).collect::<String>();
    output.push('…');
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造测试片段。
    fn fragment(kind: FragmentKind, content: &str, preview: Option<&str>) -> Fragment {
        Fragment {
            id: format!("{}_1", kind.as_str()),
            kind,
            source: "system".into(),
            description: "编写迁移时使用".into(),
            content: content.into(),
            preview: preview.map(str::to_string),
        }
    }

    /// 只命中片段时仍产出完整结构，工具与 Skill 为空数组。
    #[test]
    fn prompt_only_selection_produces_structured_detail() {
        let prompt = fragment(FragmentKind::Prompt, "迁移必须可回退", None);
        let value: Value =
            serde_json::from_str(&preselect_detail(None, [&prompt].into_iter())).unwrap();
        assert_eq!(value["ok"], json!(true));
        assert_eq!(value["router"], json!("jev"));
        assert_eq!(value["tools"], json!([]));
        assert_eq!(value["contexts"][0]["kind"], json!("prompt"));
        assert_eq!(value["contexts"][0]["preview"], json!("迁移必须可回退"));
    }

    /// 工具暴露结果保留原字段，片段追加在 contexts。
    #[test]
    fn announced_tools_are_kept_alongside_contexts() {
        let announced = r#"{"ok":true,"router":"jev","tools":[{"name":"web_search"}],"skills":[]}"#;
        let memory = fragment(
            FragmentKind::Memory,
            "契约正文",
            Some("<memory>\n说明句。\n\n项目记忆：\n- [偏好](a.md) — 用 pnpm\n</memory>"),
        );
        let value: Value =
            serde_json::from_str(&preselect_detail(Some(announced), [&memory].into_iter()))
                .unwrap();
        assert_eq!(value["tools"][0]["name"], json!("web_search"));
        let preview = value["contexts"][0]["preview"].as_str().unwrap();
        assert_eq!(preview, "项目记忆：\n- [偏好](a.md) — 用 pnpm");
        assert!(!preview.contains("契约正文"));
    }

    /// 超长片段在界面预览中截断。
    #[test]
    fn long_previews_are_truncated() {
        let long = "x".repeat(PREVIEW_CHARS + 50);
        let prompt = fragment(FragmentKind::Prompt, &long, None);
        let value: Value =
            serde_json::from_str(&preselect_detail(None, [&prompt].into_iter())).unwrap();
        let preview = value["contexts"][0]["preview"].as_str().unwrap();
        assert_eq!(preview.chars().count(), PREVIEW_CHARS + 1);
    }
}
