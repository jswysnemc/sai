use super::{optional_string_arg, MeshContext};
use crate::i18n::text as t;
use crate::tools::{ToolRegistry, ToolSpec};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

/// 英文版工具说明。
pub(super) const DESCRIPTION_EN: &str = "Create a session in the explicitly specified workdir and return its id and address. This only registers a session: it does not run an Agent, send a task, activate the session, or change any current-session or next-launch pointer. Use an existing directory. Optional title sets a display label. To forward to an existing session, use session_probe and mesh_send instead of creating one. Do not call session_activate as a follow-up to creation.";

/// 中文版工具说明。
pub(super) const DESCRIPTION_ZH: &str = "在明确指定的 workdir 下创建会话，返回 id 与通信地址。只登记会话，不运行 Agent、不投递任务、不激活，也不改变当前会话或下次启动指针。workdir 必须是已存在目录。可选 title 仅设置显示名称。向现有会话转发时使用 session_probe 与 mesh_send，不要另建会话。创建后不要顺带调用 session_activate。";

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
                    "workdir": {
                        "type": "string",
                        "minLength": 1,
                        "description": "Existing working directory for the new session. Relative paths resolve from the caller's workdir. No session pointer is changed."
                    },
                    "title": {
                        "type": "string",
                        "description": t(
                            "Optional explicit session name. Omit to keep auto-titling from the first message.",
                            "可选的显式会话名。省略则保持从第一条消息自动生成标题。"
                        )
                    }
                },
                "required": ["workdir"],
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
    let workdir = super::required_string_arg(&args, "workdir")?;
    let workdir = crate::tools::fs_path::expand_path(&workdir);
    let workdir = if workdir.is_absolute() {
        workdir
    } else {
        crate::runtime_cwd::current_dir()?.join(workdir)
    };
    let workdir = crate::platform::windows_path::canonicalize(&workdir)
        .context("session_create workdir is unavailable")?;
    if !workdir.is_dir() {
        bail!("session_create workdir must be an existing directory");
    }
    let title = optional_string_arg(&args, "title");
    let session = crate::state::create_session_detached_for_workspace(
        &context.paths,
        &workdir,
        title.as_deref(),
    )?;
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "session_id": session.id,
        "title": session.title,
        "workdir": workdir.display().to_string(),
        "address": format!("session:{}", session.id),
        "activated": false,
        "started": false,
        "pointer_changed": false,
        "note": t(
            "Created in workdir without changing current-session or next-launch pointers. No Agent was started. Open this session explicitly with sai resume <id> when needed.",
            "已在 workdir 下创建，不改变当前会话或下次启动指针，也未启动 Agent。需要打开时显式执行 sai resume <id>。"
        ),
    }))?)
}
