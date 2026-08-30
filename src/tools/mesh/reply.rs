use super::mailbox::{self, MeshEnvelope, KIND_REPLY};
use super::{required_string_arg, unix_millis, MeshAddress, MeshContext};
use crate::i18n::text as t;
use crate::tools::{ToolRegistry, ToolSpec};
use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};
use std::path::PathBuf;

/// 注册 mesh_reply 工具。
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
            "mesh_reply",
            t(
                "Answer a message received by this session. Pass the correlation_id from mesh_recv (or the one mesh_send reported) plus the reply text. The reply goes back to the original sender's reply_to address and carries the same correlation_id, so a mesh_send call waiting with expect_reply=true receives it. You can only reply to a message that is actually sitting in your own inbox, which is what stops forging someone else's thread. Replying to another session requires mesh.cross_session=true.",
                "回复本会话收到的一条消息。传入 mesh_recv(或 mesh_send 返回的)correlation_id 与回复正文。回复按原始消息的 reply_to 地址送回发送方,并带上同一个 correlation_id,正以 expect_reply=true 等待的 mesh_send 会收到它。只能回复确实躺在本会话信箱里的消息,这一点杜绝了伪造别人的会话线程。回复给别的会话需要 mesh.cross_session=true。",
            ),
            json!({
                "type": "object",
                "properties": {
                    "correlation_id": {
                        "type": "string",
                        "description": t("Correlation id of the message being answered.", "正在回复的那条消息的关联 id。")
                    },
                    "text": {
                        "type": "string",
                        "description": t("Reply body.", "回复正文。")
                    }
                },
                "required": ["correlation_id", "text"],
                "additionalProperties": false
            }),
            move |args| {
                let context = context.clone();
                async move { reply(context, args).await }
            },
        )
        .writes(),
    );
}

/// 回复本会话收到的一条消息。
///
/// 参数:
/// - `context`: 网格上下文
/// - `args`: 工具参数
///
/// 返回:
/// - JSON 形式的投递结果
pub(super) async fn reply(context: MeshContext, args: Value) -> Result<String> {
    let correlation_id = required_string_arg(&args, "correlation_id")?;
    let text = required_string_arg(&args, "text")?;
    if text.trim().is_empty() {
        bail!("mesh_reply requires non-empty text");
    }

    let state_dir = PathBuf::from(&context.owner_key);
    // 只能在自己信箱里找到的消息上回复：既拿到了回复地址，也防止伪造别人的关联 id
    let original = mailbox::find(&state_dir, &correlation_id).ok_or_else(|| {
        anyhow!("no mesh message with correlation_id={correlation_id} in this session inbox")
    })?;
    let target = MeshAddress::parse(original.reply_to.as_deref().unwrap_or(&original.from))?;
    // 归属校验：回复给别的会话同样需要 mesh.cross_session
    super::authorize_target(&context, &target)?;

    let from = super::self_address(&context).wire();
    let queued_at_ms = unix_millis();
    let envelope = MeshEnvelope {
        id: mailbox::new_id("msg"),
        correlation_id: Some(correlation_id),
        reply_to: None,
        from,
        to: target.wire(),
        kind: KIND_REPLY.to_string(),
        text,
        queued_at_ms,
        pid: std::process::id(),
        heartbeat_at: queued_at_ms,
    };
    let delivered_to = mailbox::deliver(&context.paths, &target, &envelope)?;

    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "correlation_id": envelope.correlation_id,
        "to": envelope.to,
        "from": envelope.from,
        "in_reply_to": original.id,
        "delivered_to": delivered_to,
        "queued_at_ms": queued_at_ms,
    }))?)
}
