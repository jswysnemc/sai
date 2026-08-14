use super::support::{library, required_str, scope_label};
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use anyhow::Result;
use serde_json::{json, Value};

/// 读取一条记忆的完整正文。
///
/// 参数:
/// - `args`: 工具入参
/// - `config`: 应用配置
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 记忆内容的 JSON 文本；不存在时返回未找到而不是报错
pub(super) async fn read_memory(args: Value, config: AppConfig, paths: SaiPaths) -> Result<String> {
    let name = required_str(&args, "name")?;
    let Some((entry, scope)) = library(&config, &paths).load(name)? else {
        return Ok(json!({ "found": false, "name": name }).to_string());
    };
    Ok(json!({
        "found": true,
        "name": entry.front.name,
        "description": entry.front.description,
        "type": entry.front.memory_type.as_str(),
        "scope": scope_label(scope),
        "content": entry.body,
        "links": entry.links(),
    })
    .to_string())
}

/// 列出全部记忆的摘要。
///
/// 参数:
/// - `_args`: 工具入参，无参数
/// - `config`: 应用配置
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 摘要列表的 JSON 文本
pub(super) async fn list_memory(_args: Value, config: AppConfig, paths: SaiPaths) -> Result<String> {
    let entries: Vec<Value> = library(&config, &paths)
        .list()?
        .into_iter()
        .map(|summary| {
            json!({
                "name": summary.name,
                "description": summary.description,
                "type": summary.memory_type.as_str(),
                "scope": scope_label(summary.scope),
            })
        })
        .collect();
    Ok(json!({ "count": entries.len(), "entries": entries }).to_string())
}

/// 返回读取工具的参数结构。
///
/// 参数:
/// - 无
///
/// 返回:
/// - JSON Schema
pub(super) fn read_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "name": {
                "type": "string",
                "description": crate::i18n::text(
                    "The memory identifier shown in the index.",
                    "索引里列出的记忆标识。"
                )
            }
        },
        "required": ["name"],
        "additionalProperties": false
    })
}

/// 返回列表工具的参数结构。
///
/// 参数:
/// - 无
///
/// 返回:
/// - JSON Schema
pub(super) fn list_schema() -> Value {
    json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false
    })
}
