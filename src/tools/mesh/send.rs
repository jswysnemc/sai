use super::mailbox::{self, MeshEnvelope, KIND_MESSAGE};
use super::{required_string_arg, unix_millis, MeshAddress, MeshContext};
use crate::i18n::text as t;
use crate::tools::{ToolRegistry, ToolSpec};
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

/// `expect_reply` 的缺省等待时长（毫秒）。
const DEFAULT_TIMEOUT_MS: u64 = 3_000;
/// `expect_reply` 允许的最大等待时长（毫秒）。
const MAX_TIMEOUT_MS: u64 = 60_000;
/// 等待回复时的轮询间隔（毫秒）。
const POLL_INTERVAL_MS: u64 = 20;

/// 注册 mesh_send 工具。
///
/// 参数:
/// - `registry`: 工具注册表
/// - `context`: 网格上下文
///
/// 返回:
/// - 无
pub(super) fn register(registry: &mut ToolRegistry, context: MeshContext) {
    registry.register(
        ToolSpec::new(
            "mesh_send",
            t(
                "Send a message to a session or a subagent. to accepts session:<session_id>, agent:<owner_key>/<agent_id>, or broadcast. Returns a correlation_id that the receiver can answer with mesh_reply. Set expect_reply=true to block until the reply arrives or timeout_ms (default 3000) elapses; the reply is returned inline and reports timed_out=true when nobody answered. Sending outside the current session requires mesh.cross_session=true, otherwise the call is rejected. Use session_probe and agent_probe to find live targets first.",
                "向会话或子智能体发送一条消息。to 支持 session:<session_id>、agent:<owner_key>/<agent_id> 或 broadcast。返回 correlation_id,接收方可用 mesh_reply 回复。expect_reply=true 时会阻塞等待回复,直到收到或超过 timeout_ms(默认 3000),回复随调用一并返回,无人回复时 timed_out=true。发给当前会话之外的目标需要 mesh.cross_session=true,否则直接拒绝。先用 session_probe 与 agent_probe 找到存活的目标。",
            ),
            json!({
                "type": "object",
                "properties": {
                    "to": {
                        "type": "string",
                        "description": t("Destination address: session:<session_id>, agent:<owner_key>/<agent_id>, or broadcast.", "目标地址:session:<session_id>、agent:<owner_key>/<agent_id> 或 broadcast。")
                    },
                    "text": {
                        "type": "string",
                        "description": t("Message body.", "消息正文。")
                    },
                    "expect_reply": {
                        "type": "boolean",
                        "description": t("Wait for the receiver to answer with mesh_reply. Defaults to false.", "等待接收方用 mesh_reply 回复。默认 false。")
                    },
                    "timeout_ms": {
                        "type": "integer",
                        "description": t("How long to wait for the reply in milliseconds. Defaults to 3000, capped at 60000.", "等待回复的毫秒数。默认 3000,上限 60000。")
                    }
                },
                "required": ["to", "text"],
                "additionalProperties": false
            }),
            move |args| {
                let context = context.clone();
                async move { send(context, args).await }
            },
        )
        .writes(),
    );
}

/// 发送一条网格消息。
///
/// 参数:
/// - `context`: 网格上下文
/// - `args`: 工具参数
///
/// 返回:
/// - JSON 形式的投递结果
pub(super) async fn send(context: MeshContext, args: Value) -> Result<String> {
    let target = MeshAddress::parse(&required_string_arg(&args, "to")?)?;
    let text = required_string_arg(&args, "text")?;
    if text.trim().is_empty() {
        bail!("mesh_send requires non-empty text");
    }
    // 归属校验：跨会话目标必须显式开启 mesh.cross_session
    super::authorize_target(&context, &target)?;

    let from = super::self_address(&context).wire();
    let correlation_id = mailbox::new_id("corr");
    let queued_at_ms = unix_millis();
    let envelope = MeshEnvelope {
        id: mailbox::new_id("msg"),
        correlation_id: Some(correlation_id.clone()),
        reply_to: Some(from.clone()),
        from,
        to: target.wire(),
        kind: KIND_MESSAGE.to_string(),
        text,
        queued_at_ms,
        pid: std::process::id(),
        heartbeat_at: queued_at_ms,
    };
    let delivered_to = mailbox::deliver(&context.paths, &target, &envelope)?;

    let expect_reply = args
        .get("expect_reply")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut result = json!({
        "ok": true,
        "correlation_id": correlation_id,
        "to": envelope.to,
        "from": envelope.from,
        "delivered_to": delivered_to,
        "queued_at_ms": queued_at_ms,
        "expect_reply": expect_reply,
    });
    if expect_reply {
        let timeout_ms = args
            .get("timeout_ms")
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_TIMEOUT_MS)
            .min(MAX_TIMEOUT_MS);
        let reply = wait_for_reply(&context, &correlation_id, timeout_ms).await;
        result["reply"] = reply
            .as_ref()
            .map(|envelope| envelope.to_json())
            .unwrap_or(Value::Null);
        result["timed_out"] = json!(reply.is_none());
    }
    Ok(serde_json::to_string_pretty(&result)?)
}

/// 轮询等待一条回复。
///
/// 同时查内存与磁盘信箱，因此本进程和别的进程回过来的回复都能收到。
///
/// 参数:
/// - `context`: 网格上下文
/// - `correlation_id`: 关联 id
/// - `timeout_ms`: 最长等待时间（毫秒）
///
/// 返回:
/// - 收到的回复；超时返回 `None`
async fn wait_for_reply(
    context: &MeshContext,
    correlation_id: &str,
    timeout_ms: u64,
) -> Option<MeshEnvelope> {
    let state_dir = std::path::PathBuf::from(&context.owner_key);
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        if let Some(reply) = mailbox::find_reply(&state_dir, correlation_id) {
            return Some(reply);
        }
        if Instant::now() >= deadline {
            return None;
        }
        tokio::time::sleep(Duration::from_millis(POLL_INTERVAL_MS)).await;
    }
}
