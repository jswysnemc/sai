use anyhow::{bail, Result};
use serde_json::Value;
use std::collections::BTreeSet;

/// `load` 支持的资源类型。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LoadType {
    Tool,
    Skill,
}

/// 归一化后的 `load` 请求。
#[derive(Debug, Eq, PartialEq)]
pub(super) struct LoadRequest {
    pub(super) resource_type: LoadType,
    pub(super) keywords: Vec<String>,
}

impl LoadRequest {
    /// 解析并归一化模型生成的 `load` 参数。
    ///
    /// 参数:
    /// - `arguments`: 模型生成的 JSON 参数
    ///
    /// 返回:
    /// - 资源类型和去重后的关键词数组
    pub(super) fn parse(arguments: &str) -> Result<Self> {
        let value = serde_json::from_str::<Value>(arguments.trim())?;
        let Some(object) = value.as_object() else {
            bail!("load arguments must be a JSON object");
        };

        // 1. 优先读取公开契约，兼容常见的单复数类型值
        let explicit_type = object
            .get("type")
            .or_else(|| object.get("kind"))
            .and_then(Value::as_str)
            .map(parse_load_type)
            .transpose()?;

        // 2. 兼容模型使用单数关键词或旧字段名，避免同一语义触发重试
        let candidates = [
            (LoadType::Tool, "keywords"),
            (LoadType::Tool, "keyword"),
            (LoadType::Tool, "tools"),
            (LoadType::Skill, "skills"),
            (LoadType::Tool, "tool_names"),
            (LoadType::Tool, "tool_name"),
            (LoadType::Skill, "skill_names"),
            (LoadType::Skill, "skill_name"),
        ];
        let mut selected_type = explicit_type;
        let mut keywords = None;
        for (fallback_type, name) in candidates {
            let Some(candidate) = object.get(name) else {
                continue;
            };
            if keywords.is_some() {
                bail!("load accepts one keywords source");
            }
            let resource_type = explicit_type.unwrap_or(fallback_type);
            if name == "tools" && resource_type != LoadType::Tool
                || name == "skills" && resource_type != LoadType::Skill
                || name.starts_with("tool_") && resource_type != LoadType::Tool
                || name.starts_with("skill_") && resource_type != LoadType::Skill
            {
                bail!("load type conflicts with {name}");
            }
            selected_type = Some(resource_type);
            keywords = Some(parse_keywords(candidate)?);
        }

        let Some(resource_type) = selected_type else {
            bail!("load requires type: tool or skill");
        };
        let Some(keywords) = keywords else {
            bail!("load requires a non-empty keywords array");
        };
        Ok(Self {
            resource_type,
            keywords,
        })
    }
}

/// 解析 `load` 资源类型。
///
/// 参数:
/// - `value`: 模型生成的类型文本
///
/// 返回:
/// - 归一化后的资源类型
fn parse_load_type(value: &str) -> Result<LoadType> {
    match value.trim().to_ascii_lowercase().as_str() {
        "tool" | "tools" => Ok(LoadType::Tool),
        "skill" | "skills" => Ok(LoadType::Skill),
        _ => bail!("load type must be tool or skill"),
    }
}

/// 解析关键词字符串或数组，并按首次出现顺序去重。
///
/// 参数:
/// - `value`: 关键词 JSON 值
///
/// 返回:
/// - 非空关键词数组
fn parse_keywords(value: &Value) -> Result<Vec<String>> {
    let values = match value {
        Value::String(value) => vec![value.as_str()],
        Value::Array(values) => values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("load keywords must contain only strings"))
            })
            .collect::<Result<Vec<_>>>()?,
        _ => bail!("load keywords must be a string or an array of strings"),
    };
    let mut seen = BTreeSet::new();
    let mut keywords = Vec::new();
    for value in values {
        let value = value.trim();
        if value.is_empty() {
            bail!("load keywords must contain only non-empty strings");
        }
        if seen.insert(value.to_string()) {
            keywords.push(value.to_string());
        }
    }
    if keywords.is_empty() {
        bail!("load requires a non-empty keywords array");
    }
    Ok(keywords)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证公开数组契约按输入顺序去重。
    #[test]
    fn parses_public_array_contract() {
        let request = LoadRequest::parse(
            r#"{"type":"tool","keywords":["web_search","web_fetch","web_search"]}"#,
        )
        .unwrap();

        assert_eq!(request.resource_type, LoadType::Tool);
        assert_eq!(request.keywords, ["web_search", "web_fetch"]);
    }

    /// 验证常见的单复数和值类型偏差可以归一化。
    #[test]
    fn tolerates_plural_type_and_string_keyword() {
        let request = LoadRequest::parse(r#"{"type":"skills","keywords":"fast-context"}"#).unwrap();

        assert_eq!(request.resource_type, LoadType::Skill);
        assert_eq!(request.keywords, ["fast-context"]);
    }

    /// 验证旧字段仍可推断资源类型，减少已有会话重试。
    #[test]
    fn tolerates_legacy_named_fields() {
        let request = LoadRequest::parse(r#"{"tool_names":["web_search","web_fetch"]}"#).unwrap();

        assert_eq!(request.resource_type, LoadType::Tool);
        assert_eq!(request.keywords, ["web_search", "web_fetch"]);
    }

    /// 验证类型与字段冲突时拒绝猜测。
    #[test]
    fn rejects_conflicting_resource_types() {
        let error = LoadRequest::parse(r#"{"type":"tool","skills":["fast-context"]}"#).unwrap_err();

        assert!(error.to_string().contains("conflicts"));
    }
}
