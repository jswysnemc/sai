use super::support::required_str;
use crate::config::AppConfig;
use crate::memory::MemoryStore;
use crate::paths::SaiPaths;
use anyhow::Result;
use serde_json::{json, Value};

/// 检索已被压缩清出上下文的对话轮次。
///
/// 压缩摘要是有损的，原文仍在库里。这条路径是摘要末尾那句回读指引
/// 所指向的能力，去掉它会让指引变成空头承诺。
///
/// 参数:
/// - `args`: 工具入参
/// - `config`: 应用配置
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 检索结果的 JSON 文本
pub(super) async fn search_evicted_context(
    args: Value,
    config: AppConfig,
    paths: SaiPaths,
) -> Result<String> {
    let query = required_str(&args, "query")?;
    let limit = args
        .get("max_results")
        .and_then(Value::as_u64)
        .unwrap_or(5)
        .clamp(1, 50) as usize;
    let store = MemoryStore::new(&config, &paths);
    Ok(store
        .search_evicted_context_readonly(query, limit)?
        .to_string())
}

/// 返回检索工具的参数结构。
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
            "query": {
                "type": "string",
                "description": crate::i18n::text(
                    "Search keywords or question.",
                    "搜索关键词或问题。"
                )
            },
            "max_results": {
                "type": "integer",
                "description": crate::i18n::text(
                    "Optional result limit.",
                    "可选结果数量限制。"
                )
            }
        },
        "required": ["query"],
        "additionalProperties": false
    })
}
