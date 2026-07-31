use crate::agent::AgentEvent;
use crate::agent_engine::EventSender;
use crate::llm::OpenAiCompatibleClient;
use crate::permission::{PermissionDecision, PermissionProfile};
use crate::tools::ToolPermission;
use anyhow::{bail, Result};
use serde_json::Value;

/// 对一次操作执行交互式权限审核。
///
/// 与原生内核走同一条通道：`requires_interactive_audit` 判定是否需要人工确认，
/// 需要则挂起等待用户在权限卡上决定，并把批准记录写回 profile ——
/// 后续的 `authorize` 正是靠这条记录放行。
///
/// 缺了这一步，audited 模式下 `authorize` 会因为找不到批准记录直接拒绝，
/// 表现为「外部内核的操作既不弹审核也不执行，而是直接失败」。
///
/// 参数:
/// - `profile`: 权限配置
/// - `session_id`: 会话标识，权限卡按此归属到当前会话
/// - `tool`: 工具名称
/// - `arguments`: 工具参数
/// - `permission`: 工具的权限等级
/// - `events`: 事件发送端，用于把权限卡呈现到界面
/// - `auto_audit`: 自动审核所需的运行时；非自动审核模式为 None
///
/// 返回:
/// - 获准或无需审核时为 Ok；被拒时返回错误
pub(crate) async fn ensure_authorized(
    profile: &PermissionProfile,
    session_id: &str,
    tool: &str,
    arguments: &Value,
    permission: ToolPermission,
    events: &EventSender,
    auto_audit: Option<&AutoAuditRuntime>,
) -> Result<()> {
    // 1. 不需要交互式审核的操作直接放行，避免为只读操作打扰用户
    if !profile.requires_interactive_audit(tool, permission, arguments) {
        return Ok(());
    }
    profile.record_requested(tool, arguments);
    let arguments_text = serde_json::to_string(arguments).unwrap_or_else(|_| "{}".to_string());
    // 2. 自动审核与人工审核并行抢答，与自带内核的行为一致
    let auto_active = auto_audit.is_some();
    let (request, receiver) = crate::permission::request_permission_with_auto_audit(
        session_id,
        tool,
        &arguments_text,
        auto_active,
    );
    let request_id = request.id.clone();
    let auto_task = auto_audit.map(|runtime| {
        let client = runtime.client.clone();
        let context = runtime.context.clone();
        let request_id = request_id.clone();
        let tool = tool.to_string();
        let arguments_text = arguments_text.clone();
        tokio::spawn(async move {
            // 失败或超时静默回退人工审核，与自带内核一致
            let _ = crate::permission::run_auto_audit(
                &client,
                &request_id,
                &tool,
                &arguments_text,
                &context,
            )
            .await;
        })
    });
    // 3. 权限卡送到界面，等待人工或自动审核给出结论
    let _ = events.send(AgentEvent::PermissionRequested(request));
    let decision = match receiver.await {
        Ok(decision) => decision,
        // 通道关闭说明会话已结束，按拒绝处理而不是放行
        Err(_) => {
            profile.record_denied(tool, arguments, Some("permission channel closed"));
            bail!("permission request was dropped before a decision")
        }
    };
    // 已有结论，未完成的自动审核不再需要
    if let Some(task) = auto_task {
        task.abort();
    }
    let _ = events.send(AgentEvent::PermissionResolved {
        request_id,
        decision: decision.clone(),
    });
    match decision {
        PermissionDecision::Allow { .. } => {
            // 3. 写入一次性批准，随后的 authorize 据此放行
            let detail = decision.detail().map(str::to_string);
            profile.record_approved(tool, arguments, detail.as_deref());
            Ok(())
        }
        PermissionDecision::Deny { reply } => {
            profile.record_denied(tool, arguments, reply.as_deref());
            let message = reply
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| {
                    crate::i18n::text("the user denied this operation", "用户拒绝了此操作")
                        .to_string()
                });
            bail!(message)
        }
    }
}

/// 自动审核所需的运行时。
///
/// 外部内核没有 sai 的对话历史，`context` 由调用方给出简要说明，
/// 让审核模型至少知道这次操作发生在什么场景下。
#[derive(Clone)]
pub(crate) struct AutoAuditRuntime {
    /// 审核模型客户端
    pub(crate) client: OpenAiCompatibleClient,
    /// 供审核模型参考的上下文摘要
    pub(crate) context: String,
}
