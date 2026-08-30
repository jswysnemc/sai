use super::mailbox;
use super::{optional_string_arg, unix_millis, MeshContext};
use crate::i18n::text as t;
use crate::tools::{ToolRegistry, ToolSpec};
use anyhow::Result;
use serde_json::{json, Value};
use std::path::PathBuf;

/// 缺省返回条数。
const DEFAULT_LIMIT: u64 = 50;
/// 单次返回的最大条数。
const MAX_LIMIT: u64 = 200;

/// 注册 mesh_recv 工具。
///
/// 参数:
/// - `registry`: 工具注册表
/// - `context`: 网格上下文
///
/// 返回:
/// - 无
pub(super) fn register(registry: &mut ToolRegistry, context: MeshContext) {
    registry.register(ToolSpec::new(
        "mesh_recv",
        t(
            "Read messages sent to this session by mesh_send. Without filters it returns the newest messages in the session inbox; pass correlation_id to read one conversation thread, since_ms to read only the last N milliseconds, and limit to cap the count. Messages are returned oldest first and are not consumed, so repeated calls see the same history. Each entry carries id, correlation_id, reply_to and from so you can answer it with mesh_reply. Read-only: it never removes or modifies a message.",
            "读取 mesh_send 发给本会话的消息。不带过滤时返回信箱里最新的消息;传 correlation_id 只看某一条会话线程,since_ms 只看最近 N 毫秒,limit 限制条数。消息按时间正序返回且不被取走,重复调用看到同样的历史。每条记录带 id、correlation_id、reply_to 与 from,可直接用 mesh_reply 回复。只读工具:不会删除或修改任何消息。",
        ),
        json!({
            "type": "object",
            "properties": {
                "correlation_id": {
                    "type": "string",
                    "description": t("Return only messages of this request-reply thread.", "只返回该请求-回复线程里的消息。")
                },
                "since_ms": {
                    "type": "integer",
                    "description": t("Return only messages queued within the last N milliseconds.", "只返回最近 N 毫秒内入队的消息。")
                },
                "limit": {
                    "type": "integer",
                    "description": t("Maximum number of messages to return. Defaults to 50, capped at 200.", "最多返回多少条。默认 50,上限 200。")
                }
            },
            "additionalProperties": false
        }),
        move |args| {
            let context = context.clone();
            async move { recv(context, args).await }
        },
    ));
}

/// 收取本会话信箱里的消息。
///
/// 参数:
/// - `context`: 网格上下文
/// - `args`: 工具参数
///
/// 返回:
/// - JSON 形式的消息列表
pub(super) async fn recv(context: MeshContext, args: Value) -> Result<String> {
    let correlation_id = optional_string_arg(&args, "correlation_id");
    let since_ms = args.get("since_ms").and_then(Value::as_u64);
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_LIMIT)
        .clamp(1, MAX_LIMIT) as usize;

    let state_dir = PathBuf::from(&context.owner_key);
    let mut messages = mailbox::list(&state_dir);
    if let Some(correlation_id) = &correlation_id {
        messages.retain(|envelope| {
            envelope.correlation_id.as_deref() == Some(correlation_id.as_str())
                || envelope.id == *correlation_id
        });
    }
    if let Some(window) = since_ms {
        let cutoff = unix_millis().saturating_sub(window);
        messages.retain(|envelope| envelope.queued_at_ms >= cutoff);
    }
    let total = messages.len();
    // 超过上限时只保留最新的若干条，但仍按时间正序返回
    if messages.len() > limit {
        messages = messages.split_off(messages.len() - limit);
    }
    let messages = messages
        .iter()
        .map(|envelope| envelope.to_json())
        .collect::<Vec<_>>();

    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "self": {
            "session_id": context.session_id,
            "state_dir": context.owner_key,
        },
        "inbox": mailbox::inbox_file(&state_dir).display().to_string(),
        "filters": {
            "correlation_id": correlation_id,
            "since_ms": since_ms,
            "limit": limit,
        },
        "total": total,
        "count": messages.len(),
        "messages": messages,
    }))?)
}
