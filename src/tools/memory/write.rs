use super::support::{library, optional_str, parse_scope, required_str, scope_label};
use crate::memory::file_store::{Frontmatter, MemoryEntry, MemoryType};
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};

/// 需要说明理由的类型必须出现的两个小标题。
const RATIONALE_MARKERS: [&str; 2] = ["Why:", "How to apply:"];

/// 写入或更新一条记忆。
///
/// 参数:
/// - `args`: 工具入参
/// - `config`: 应用配置
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 写入结果的 JSON 文本
pub(super) async fn write_memory(args: Value, config: AppConfig, paths: SaiPaths) -> Result<String> {
    let name = required_str(&args, "name")?.to_string();
    let description = required_str(&args, "description")?.to_string();
    let content = required_str(&args, "content")?.to_string();
    let raw_type = required_str(&args, "type")?;
    let memory_type = MemoryType::parse(raw_type)
        .ok_or_else(|| anyhow!("未知记忆类型：{raw_type}，可选 user、feedback、project、reference"))?;
    let hook = match optional_str(&args, "hook") {
        "" => description.clone(),
        value => value.to_string(),
    };
    let scope = parse_scope(&args);
    let entry = MemoryEntry {
        front: Frontmatter {
            name: name.clone(),
            description,
            memory_type,
        },
        body: content,
    };
    let library = library(&config, &paths);
    let existed = library.load(&name)?.is_some();
    library.save(scope, &entry, &hook)?;
    let mut result = json!({
        "ok": true,
        "name": name,
        "scope": scope_label(scope),
        "updated": existed,
        "links": entry.links(),
    });
    // 缺理由不阻止写入，但要让模型知道下次该补：直接失败会让它反复重试
    if let Some(missing) = missing_rationale(memory_type, &entry.body) {
        result["note"] = json!(missing);
    }
    Ok(result.to_string())
}

/// 检查需要理由的类型是否写全了理由与应用方式。
///
/// 参数:
/// - `memory_type`: 条目类型
/// - `body`: 正文
///
/// 返回:
/// - 缺失提示；无需理由或已写全时为 None
fn missing_rationale(memory_type: MemoryType, body: &str) -> Option<String> {
    if !memory_type.requires_rationale() {
        return None;
    }
    let missing: Vec<&str> = RATIONALE_MARKERS
        .iter()
        .filter(|marker| !body.contains(**marker))
        .copied()
        .collect();
    if missing.is_empty() {
        return None;
    }
    Some(format!(
        "{} 类记忆建议在正文补上 {}：缺了理由，下一轮无法判断它在新情境下还适不适用。",
        memory_type.as_str(),
        missing.join(" 与 ")
    ))
}

/// 返回写入工具的参数结构。
///
/// 参数:
/// - 无
///
/// 返回:
/// - JSON Schema
pub(super) fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "name": {
                "type": "string",
                "description": crate::i18n::text(
                    "Short kebab-case identifier, also the file name and link target.",
                    "短横线分隔的标识，同时是文件名与关联目标。"
                )
            },
            "description": {
                "type": "string",
                "description": crate::i18n::text(
                    "One-line summary used to judge relevance during recall.",
                    "一句话摘要，召回时据此判断相关性。"
                )
            },
            "type": {
                "type": "string",
                "enum": ["user", "feedback", "project", "reference"],
                "description": crate::i18n::text(
                    "user: who the user is. feedback: how you should work, include the why. project: ongoing work and constraints not derivable from the code. reference: pointers to external resources.",
                    "user：用户是谁。feedback：工作方式要求，需写明理由。project：进行中的工作与约束，且无法从代码看出。reference：外部资源指针。"
                )
            },
            "content": {
                "type": "string",
                "description": crate::i18n::text(
                    "The fact itself. For feedback and project, follow with Why: and How to apply: lines. Link related memories with [[name]].",
                    "事实本身。feedback 与 project 类需补 Why: 与 How to apply: 两行。用 [[name]] 关联其它记忆。"
                )
            },
            "hook": {
                "type": "string",
                "description": crate::i18n::text(
                    "Optional one-line hook shown in the index; defaults to the description.",
                    "可选，索引行里显示的提示；默认用摘要。"
                )
            },
            "scope": {
                "type": "string",
                "enum": ["project", "global"],
                "description": crate::i18n::text(
                    "project (default) keeps it to the current workspace; global applies everywhere.",
                    "project（默认）只在当前工作区生效；global 处处生效。"
                )
            }
        },
        "required": ["name", "description", "type", "content"],
        "additionalProperties": false
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证需要理由的类型缺小标题时给出提示。
    #[test]
    fn a_feedback_without_rationale_is_flagged() {
        let note = missing_rationale(MemoryType::Feedback, "一律使用 pnpm").unwrap();

        assert!(note.contains("Why:"));
        assert!(note.contains("How to apply:"));
    }

    /// 验证写全理由后不再提示。
    #[test]
    fn a_complete_feedback_passes() {
        let body = "一律使用 pnpm\n\n**Why:** 锁文件不能混用\n**How to apply:** 装依赖时用 pnpm add";

        assert!(missing_rationale(MemoryType::Feedback, body).is_none());
    }

    /// 验证不需要理由的类型从不提示。
    #[test]
    fn types_without_a_rationale_requirement_are_never_flagged() {
        assert!(missing_rationale(MemoryType::User, "用户是 Rust 开发者").is_none());
        assert!(missing_rationale(MemoryType::Reference, "看板：http://x").is_none());
    }

    /// 验证只缺一个小标题时只提示那一个。
    #[test]
    fn only_the_missing_marker_is_reported() {
        let note = missing_rationale(MemoryType::Project, "目标\n\n**Why:** 因为").unwrap();

        assert!(note.contains("How to apply:"));
        assert!(!note.contains("Why: 与"));
    }

    /// 验证参数结构声明了全部必填项。
    #[test]
    fn the_schema_requires_the_essential_fields() {
        let schema = schema();
        let required = schema["required"].as_array().unwrap();

        for field in ["name", "description", "type", "content"] {
            assert!(required.iter().any(|value| value == field), "缺少 {field}");
        }
    }
}
