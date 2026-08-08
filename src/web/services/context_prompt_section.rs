use crate::i18n::Locale;
use serde::Serialize;

/// 上下文预览中的稳定分区。
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ContextPromptSection {
    /// 不随语言和标题变化的导航标识
    pub id: String,
    /// 当前界面语言下的短标签
    pub label: String,
    /// 包含标题的完整 Markdown
    pub content: String,
}

/// 构造上下文预览分区。
///
/// 参数:
/// - `id`: 稳定导航标识
/// - `label`: 顶部短标签
/// - `title`: Markdown 标题
/// - `body`: 分区正文
///
/// 返回:
/// - 可独立渲染和定位的上下文分区
pub(crate) fn section(
    id: &str,
    label: impl Into<String>,
    title: &str,
    body: &str,
) -> ContextPromptSection {
    ContextPromptSection {
        id: id.to_string(),
        label: label.into(),
        content: format!("## {title}\n\n{}", body.trim()),
    }
}

/// 构造当前模式说明的可读预览。
///
/// 参数:
/// - `mode`: 当前 Agent 模式
/// - `locale`: 当前界面语言
///
/// 返回:
/// - 当前模式名称及实际模式提示词
pub(crate) fn mode_preview(mode: crate::agent::AgentMode, locale: Locale) -> String {
    format!(
        "{}: `{}`\n\n```xml\n{}\n```",
        locale.text("Current mode", "当前模式"),
        mode.label(),
        mode.reminder().trim()
    )
}

/// 从文本中拆出一个完整 XML 标签区块。
///
/// 参数:
/// - `source`: 原始系统提示词
/// - `tag`: 不含尖括号的标签名
///
/// 返回:
/// - 移除区块后的文本与被拆出的完整区块
pub(super) fn split_tagged_section(source: &str, tag: &str) -> (String, String) {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let Some(start) = source.find(&open) else {
        return (source.to_string(), String::new());
    };
    let Some(relative_end) = source[start..].find(&close) else {
        return (source.to_string(), String::new());
    };
    let end = start + relative_end + close.len();
    let extracted = source[start..end].to_string();
    let mut remaining = source[..start].trim_end().to_string();
    let tail = source[end..].trim_start();
    if !remaining.is_empty() && !tail.is_empty() {
        remaining.push_str("\n\n");
    }
    remaining.push_str(tail);
    (remaining, extracted)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证分区标识与本地化标题相互独立。
    #[test]
    fn section_keeps_stable_id() {
        let value = section("runtime", "运行时", "4. 运行时上下文", "body");
        assert_eq!(value.id, "runtime");
        assert_eq!(value.label, "运行时");
        assert!(value.content.starts_with("## 4. 运行时上下文"));
    }

    /// 验证模式预览包含当前模式与完整说明。
    #[test]
    fn mode_preview_contains_instruction() {
        let value = mode_preview(crate::agent::AgentMode::Plan, Locale::Zh);
        assert!(value.contains("`PLAN`"));
        assert!(value.contains("<mode name=\"plan\""));
    }

    /// 验证 XML 分区拆出后不改变其余提示词顺序。
    #[test]
    fn split_tagged_section_preserves_surrounding_text() {
        let source = "before\n\n<available-skills>\ndemo\n</available-skills>\n\nafter";
        let (remaining, extracted) = split_tagged_section(source, "available-skills");
        assert_eq!(remaining, "before\n\nafter");
        assert!(extracted.contains("demo"));
    }
}
