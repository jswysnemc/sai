use super::support::{library, required_str};
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use anyhow::Result;
use serde_json::{json, Value};

/// 删除一条记忆。
///
/// 参数:
/// - `args`: 工具入参
/// - `config`: 应用配置
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 删除结果的 JSON 文本
pub(super) async fn delete_memory(args: Value, config: AppConfig, paths: SaiPaths) -> Result<String> {
    let name = required_str(&args, "name")?;
    let deleted = library(&config, &paths).delete(name)?;
    Ok(json!({ "deleted": deleted, "name": name }).to_string())
}

/// 返回删除工具的参数结构。
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
                    "The memory identifier to delete.",
                    "要删除的记忆标识。"
                )
            }
        },
        "required": ["name"],
        "additionalProperties": false
    })
}
