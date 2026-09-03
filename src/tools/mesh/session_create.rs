use super::{optional_string_arg, MeshContext};
use crate::i18n::text as t;
use crate::tools::{ToolRegistry, ToolSpec};
use anyhow::Result;
use serde_json::{json, Value};

/// 英文版工具说明。
pub(super) const DESCRIPTION_EN: &str = "Create a new session in the current workspace for team-like coordination and return its id immediately. The new session is NOT activated: the current session stays unchanged. Pass title only when the session needs an explicit name; omit it and the session keeps auto-titling from its first message. The result carries session_id and the wire address session:<session_id> — pass that address to mesh_send to deliver messages into the session. Activate it with session_activate when the user should /resume it. Sessions are keyed by id; titles are display labels only.";

/// 中文版工具说明。
pub(super) const DESCRIPTION_ZH: &str = "在当前工作区创建一个新会话（用于类团队协作）并立即返回其 id。新会话不会被激活：当前会话保持不变。只有需要显式命名时才传 title；省略时该会话保持从第一条消息自动生成标题的行为。结果带 session_id 与线格式地址 session:<session_id>——把该地址传给 mesh_send 即可向该会话投递消息。当希望用户 /resume 进入该会话时，再用 session_activate 激活它。会话以 id 为键，标题只是展示标签。";

/// 注册 session_create 工具。
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
            "session_create",
            t(DESCRIPTION_EN, DESCRIPTION_ZH),
            json!({
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": t(
                            "Optional explicit session name. Omit to keep auto-titling from the first message.",
                            "可选的显式会话名。省略则保持从第一条消息自动生成标题。"
                        )
                    }
                },
                "additionalProperties": false
            }),
            move |args| {
                let context = context.clone();
                async move { create(context, args).await }
            },
        )
        .writes(),
    );
}

/// 创建一个新会话，不改变当前会话。
///
/// 参数:
/// - `context`: 网格上下文
/// - `args`: 工具参数
///
/// 返回:
/// - JSON 形式的新会话信息
pub(super) async fn create(context: MeshContext, args: Value) -> Result<String> {
    let title = optional_string_arg(&args, "title");
    let session = crate::state::create_session_detached(&context.paths, title.as_deref())?;
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "session_id": session.id,
        "title": session.title,
        "address": format!("session:{}", session.id),
        "activated": false,
        "note": t(
            "Created but NOT activated: the current session is unchanged. Deliver messages with mesh_send to the address above; run session_activate when the user should /resume this session.",
            "已创建但未激活：当前会话没有变化。用 mesh_send 向上面的地址投递消息；当希望用户 /resume 进入该会话时再执行 session_activate。"
        ),
    }))?)
}
