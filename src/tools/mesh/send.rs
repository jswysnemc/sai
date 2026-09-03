use super::mailbox::{self, MeshEnvelope, KIND_MESSAGE};
use super::{optional_string_arg, required_string_arg, unix_millis, MeshAddress, MeshContext};
use crate::i18n::text as t;
use crate::tools::{ToolRegistry, ToolSpec};
use anyhow::{bail, Result};
use serde_json::{json, Value};

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
                "Send a message to a session or a subagent and return immediately. Do not wait for the receiver. to accepts session:<session_id>, agent:<owner_key>/<agent_id>, or broadcast. The receiver is woken through the session queue. When the receiver finishes, they send results back with another mesh_send to this call's `from` (or `reply_to`) address; pass the same correlation_id so the original sender can match the thread. Sending outside the current session requires mesh.cross_session=true. Use session_probe and agent_probe to find live targets first, including idle sessions that have a live holder but no messages yet.",
                "向会话或子智能体发送一条消息并立即返回，不要等待对方。to 支持 session:<session_id>、agent:<owner_key>/<agent_id> 或 broadcast。接收方由会话队列主动唤醒。对方完成任务后再用 mesh_send 把结果发回本次返回的 from（或 reply_to）地址，并带上同一个 correlation_id 以便对上线程。发给当前会话之外的目标需要 mesh.cross_session=true。先用 session_probe 与 agent_probe 找到存活目标，包括已打开但还没有发过消息的空闲会话。",
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
                    "correlation_id": {
                        "type": "string",
                        "description": t("Optional thread id. Omit to mint a new one; pass the inbound correlation_id when sending a result back.", "可选线程 id。省略则新生成；把结果送回去时传入入站消息的 correlation_id。")
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

/// 发送一条网格消息并立即返回。
///
/// 参数:
/// - `context`: 网格上下文
/// - `args`: 工具参数
///
/// 返回:
/// - JSON 形式的投递结果；不含等待回复
pub(super) async fn send(context: MeshContext, args: Value) -> Result<String> {
    let target = MeshAddress::parse(&required_string_arg(&args, "to")?)?;
    let text = required_string_arg(&args, "text")?;
    if text.trim().is_empty() {
        bail!("mesh_send requires non-empty text");
    }
    // 归属校验：跨会话目标必须显式开启 mesh.cross_session
    super::authorize_target(&context, &target)?;

    let from = super::self_address(&context).wire();
    let correlation_id = optional_string_arg(&args, "correlation_id")
        .unwrap_or_else(|| mailbox::new_id("corr"));
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

    // 自发检测：from == to 只可能是把 session_probe 里 is_self 那条当成了目标。
    // 静默投递成功会让模型以为消息已发给对方，实际是发给自己造成回环。
    let warning = if envelope.from == envelope.to {
        Some(t(
            "DELIVERED TO YOURSELF (from == to). You picked the session_probe entry with is_self: true — that entry is YOU. If you meant another session, re-run session_probe and choose an entry with is_self: false. Do not use is_workspace_current to decide identity.",
            "消息发给了你自己(from == to)。你选中的是 session_probe 里 is_self 为 true 的条目——那就是你自己的会话。如果想发给别的会话,重新运行 session_probe 并选择 is_self 为 false 的条目。不要用 is_workspace_current 判断身份。"
        ))
    } else {
        None
    };

    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "correlation_id": correlation_id,
        "to": envelope.to,
        "from": envelope.from,
        "reply_to": envelope.reply_to,
        "delivered_to": delivered_to,
        "queued_at_ms": queued_at_ms,
        "warning": warning,
    }))?)
}
