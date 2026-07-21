use super::{ToolRegistry, ToolSpec};
use crate::config::AppConfig;
use crate::i18n::text as t;
use crate::memory::MemoryStore;
use crate::paths::SaiPaths;
use anyhow::{bail, Result};
use serde_json::{json, Value};

pub fn register(registry: &mut ToolRegistry, config: AppConfig, paths: SaiPaths) {
    if !config.memory_config().enabled {
        return;
    }
    register_readonly(registry, config.clone(), paths.clone());
    registry.register(ToolSpec::new(
        "remember_fact",
        t("Save a durable memory fact or useful knowledge point for future association. Use only for reusable facts, preferences, methods, or stable discoveries.", "保存长期记忆事实或有用知识点，供之后联想使用。仅用于可复用事实、偏好、方法或稳定发现。"),
        json!({
            "type": "object",
            "properties": {
                "content": { "type": "string", "description": t("The concise fact or knowledge point to remember.", "要记住的简洁事实或知识点。") },
                "source": { "type": "string", "description": t("Optional source label.", "可选来源标签。") },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": t("Optional retrieval tags, e.g. niri, input-method.", "可选检索标签，例如 niri、input-method。")
                }
            },
            "required": ["content"],
            "additionalProperties": false
        }),
        {
            let config = config.clone();
            let paths = paths.clone();
            move |args| {
                let config = config.clone();
                let paths = paths.clone();
                async move { remember_fact(args, config, paths).await }
            }
        },
    ).writes());
}

pub fn register_readonly(registry: &mut ToolRegistry, config: AppConfig, paths: SaiPaths) {
    if !config.memory_config().enabled {
        return;
    }
    registry.register(ToolSpec::new(
        "search_evicted_context",
        t("Search conversation turns that were moved out of the active context window. Use this when the current context appears to be missing earlier discussion.", "搜索已经移出当前上下文窗口的对话轮次。当当前上下文明显缺少早前讨论时使用。"),
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": t("Search keywords or question.", "搜索关键词或问题。") },
                "max_results": { "type": "integer", "description": t("Optional result limit.", "可选结果数量限制。") }
            },
            "required": ["query"],
            "additionalProperties": false
        }),
        {
            let config = config.clone();
            let paths = paths.clone();
            move |args| {
                let config = config.clone();
                let paths = paths.clone();
                async move { search_evicted_context(args, config, paths).await }
            }
        },
    ));
    registry.register(ToolSpec::new(
        "recall_past_events",
        t("Search the assistant's diary-like memory of things that happened in previous conversations.", "搜索助手对过往对话事件的日记式记忆。"),
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": t("Search keywords or question.", "搜索关键词或问题。") },
                "max_results": { "type": "integer", "description": t("Optional result limit.", "可选结果数量限制。") }
            },
            "required": ["query"],
            "additionalProperties": false
        }),
        {
            let config = config.clone();
            let paths = paths.clone();
            move |args| {
                let config = config.clone();
                let paths = paths.clone();
                async move { recall_past_events(args, config, paths).await }
            }
        },
    ));
    registry.register(ToolSpec::new(
        "recall_memories",
        t("Search remembered facts and past events, including forgotten memories when requested. This read-only tool does not change memory state.", "搜索已记住的事实和过往事件；需要时也可包含已遗忘记忆。此只读工具不会改变记忆状态。"),
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": t("Search keywords or question.", "搜索关键词或问题。") },
                "max_results": { "type": "integer", "description": t("Optional result limit.", "可选结果数量限制。") },
                "include_forgotten": { "type": "boolean", "description": t("Whether to include forgotten memories.", "是否包含已遗忘记忆。") },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": t("Optional tags to bias or filter memory search.", "可选标签，用于偏置或过滤记忆检索。")
                }
            },
            "required": ["query"],
            "additionalProperties": false
        }),
        {
            let config = config.clone();
            let paths = paths.clone();
            move |args| {
                let config = config.clone();
                let paths = paths.clone();
                async move { recall_memories(args, config, paths).await }
            }
        },
    ));
}

async fn search_evicted_context(args: Value, config: AppConfig, paths: SaiPaths) -> Result<String> {
    let query = required_str(&args, "query")?;
    let limit = optional_limit(&args);
    let store = MemoryStore::new(&config, &paths);
    Ok(store
        .search_evicted_context_readonly(query, limit)?
        .to_string())
}

async fn recall_past_events(args: Value, config: AppConfig, paths: SaiPaths) -> Result<String> {
    let query = required_str(&args, "query")?;
    let limit = optional_limit(&args);
    let store = MemoryStore::new(&config, &paths);
    Ok(store.recall_past_events_readonly(query, limit)?.to_string())
}

async fn remember_fact(args: Value, config: AppConfig, paths: SaiPaths) -> Result<String> {
    let content = required_str(&args, "content")?;
    let source = args
        .get("source")
        .and_then(Value::as_str)
        .unwrap_or("conversation");
    let tags = args
        .get("tags")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let store = MemoryStore::new(&config, &paths);
    let id = store.remember_fact_with_tags(content, source, &tags)?;
    Ok(json!({ "ok": true, "id": id, "tags": tags }).to_string())
}

async fn recall_memories(args: Value, config: AppConfig, paths: SaiPaths) -> Result<String> {
    let query = required_str(&args, "query")?;
    let limit = optional_limit(&args);
    let include_forgotten = args
        .get("include_forgotten")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let tag_query = args
        .get("tags")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    let effective_query = if tag_query.is_empty() {
        query.to_string()
    } else {
        format!("{query} {tag_query}")
    };
    let store = MemoryStore::new(&config, &paths);
    Ok(store
        .recall_memories_readonly(&effective_query, limit, include_forgotten)?
        .to_string())
}

fn required_str<'a>(args: &'a Value, name: &str) -> Result<&'a str> {
    let value = args
        .get(name)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if value.is_empty() {
        bail!("{}: {name}", t("required argument missing", "缺少必需参数"));
    }
    Ok(value)
}

fn optional_limit(args: &Value) -> usize {
    args.get("max_results")
        .and_then(Value::as_u64)
        .unwrap_or(5)
        .clamp(1, 50) as usize
}
