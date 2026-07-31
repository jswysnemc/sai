use super::client_terminal::TerminalRegistry;
use super::governance::AcpGovernance;
use super::protocol::{RequestId, METHOD_NOT_FOUND};
use super::transport::AcpTransport;
use crate::agent_engine::EventSender;
use agent_client_protocol::schema::v1::{
    PermissionOption, PermissionOptionKind, ReadTextFileRequest, ReadTextFileResponse,
    RequestPermissionOutcome, RequestPermissionRequest, RequestPermissionResponse,
    SelectedPermissionOutcome, WriteTextFileRequest, WriteTextFileResponse,
};
use anyhow::Result;
use serde_json::{json, Value};

/// 处理外部 agent 反向发起的请求。
///
/// 这是 sai 的治理层生效的地方：外部内核想读写文件、执行命令或取得授权时，
/// 都要经由 sai，从而继续受权限判定、沙箱与审计日志约束，
/// 而不是绕过 sai 直接操作工作区。
///
/// 参数:
/// - `transport`: 连接
/// - `id`: 对端请求标识
/// - `method`: 方法名
/// - `params`: 参数
/// - `events`: 事件发送端，用于把授权请求呈现到界面
/// - `governance`: 治理句柄，文件写入与命令执行都要过它
/// - `terminals`: 终端集合
///
/// 返回:
/// - 处理结果；回包失败才向上传播
pub(crate) async fn handle_peer_request(
    transport: &AcpTransport,
    id: &RequestId,
    method: &str,
    params: &Value,
    events: &EventSender,
    governance: &AcpGovernance,
    terminals: &TerminalRegistry,
) -> Result<()> {
    match method {
        "session/request_permission" => {
            let result = request_permission(params, events, governance.session_id())
                .await
                .and_then(|response| super::sdk::to_value(&response));
            respond_with(transport, id, result).await
        }
        "fs/read_text_file" => {
            let result = read_text_file(params, governance, events)
                .await
                .and_then(|content| super::sdk::to_value(&ReadTextFileResponse::new(content)));
            respond_with(transport, id, result).await
        }
        "fs/write_text_file" => {
            let written = write_text_file(params, governance, events)
                .await
                .and_then(|_| super::sdk::to_value(&WriteTextFileResponse::new()));
            respond_with(transport, id, written).await
        }
        "terminal/create" => {
            let created = terminals
                .create(params, governance, events)
                .await
                .map(|id| json!({ "terminalId": id }));
            respond_with(transport, id, created).await
        }
        "terminal/output" => {
            let result = match terminal_id(params) {
                Ok(target) => terminals.output(&target).await,
                Err(error) => Err(error),
            };
            respond_with(transport, id, result).await
        }
        "terminal/wait_for_exit" => {
            let result = match terminal_id(params) {
                Ok(target) => terminals.wait_for_exit(&target, governance).await,
                Err(error) => Err(error),
            };
            respond_with(transport, id, result).await
        }
        "terminal/kill" => {
            let result = match terminal_id(params) {
                Ok(target) => terminals.kill(&target).await.map(|_| json!({})),
                Err(error) => Err(error),
            };
            respond_with(transport, id, result).await
        }
        "terminal/release" => {
            let result = match terminal_id(params) {
                Ok(target) => terminals.release(&target).await.map(|_| json!({})),
                Err(error) => Err(error),
            };
            respond_with(transport, id, result).await
        }
        "elicitation/create" => {
            let result = super::elicitation::handle(params, events, governance.session_id()).await;
            respond_with(transport, id, result).await
        }
        // 其余客户端能力尚未接入：明确回 method not found，
        // agent 会退回自己的实现或跳过，不会静默等待
        _ => {
            transport
                .respond_error(id, METHOD_NOT_FOUND, "method is not supported by sai yet")
                .await
        }
    }
}

/// 按处理结果回包。
///
/// 失败一律以错误响应回给 agent：被 sai 的权限拦下时，
/// agent 应当看到明确的拒绝而不是空结果，否则它会以为操作成功了。
///
/// 参数:
/// - `transport`: 连接
/// - `id`: 对端请求标识
/// - `result`: 处理结果
///
/// 返回:
/// - 回包结果
async fn respond_with(
    transport: &AcpTransport,
    id: &RequestId,
    result: Result<Value>,
) -> Result<()> {
    match result {
        Ok(value) => transport.respond(id, value).await,
        Err(error) => {
            transport
                .respond_error(id, REQUEST_FAILED, &format!("{error:#}"))
                .await
        }
    }
}

/// 取出请求中的终端标识。
///
/// 参数:
/// - `params`: 请求参数
///
/// 返回:
/// - 终端标识
fn terminal_id(params: &Value) -> Result<String> {
    params
        .get("terminalId")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("terminal request requires terminalId"))
}

/// 写入文本文件。
///
/// 先过 sai 的权限判定再落盘：工作区边界与符号链接逃逸的判定
/// 与自带的 write_file 工具完全一致。
///
/// 参数:
/// - `params`: `fs/write_text_file` 参数
/// - `governance`: 治理句柄
/// - `events`: 事件发送端，用于呈现权限卡
///
/// 返回:
/// - 写入结果
async fn write_text_file(
    params: &Value,
    governance: &AcpGovernance,
    events: &EventSender,
) -> Result<()> {
    let request: WriteTextFileRequest =
        super::sdk::from_value(params.clone(), "fs/write_text_file request")?;
    governance.authorize_write(&request.path, events).await?;
    if let Some(parent) = request.path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&request.path, request.content)?;
    Ok(())
}

/// 请求被拒或执行失败的错误码。
const REQUEST_FAILED: i64 = -32000;

/// 走 sai 的权限系统处理授权请求。
///
/// 复用与原生内核完全相同的通道：界面上出现同一张权限卡，
/// 自动审核照常并行抢答，决定同样写入审计日志。
///
/// 参数:
/// - `params`: `session/request_permission` 参数
/// - `events`: 事件发送端
/// - `host_session_id`: Sai 宿主会话标识
///
/// 返回:
/// - 标准 ACP 权限响应
async fn request_permission(
    params: &Value,
    events: &EventSender,
    host_session_id: &str,
) -> Result<RequestPermissionResponse> {
    let request: RequestPermissionRequest =
        super::sdk::from_value(params.clone(), "session/request_permission request")?;
    let tool = request
        .tool_call
        .fields
        .title
        .clone()
        .unwrap_or_else(|| "tool".to_string());
    let arguments = request
        .tool_call
        .fields
        .raw_input
        .as_ref()
        .map(|input| serde_json::to_string(input).unwrap_or_default())
        .unwrap_or_else(|| "{}".to_string());
    let (pending, receiver) =
        crate::permission::request_permission(host_session_id, &tool, &arguments);
    let request_id = pending.id.clone();
    let _ = events.send(crate::agent::AgentEvent::PermissionRequested(pending));
    let decision = match receiver.await {
        Ok(decision) => decision,
        // 通道关闭说明会话已结束，按取消处理让 agent 收敛
        Err(_) => {
            return Ok(RequestPermissionResponse::new(
                RequestPermissionOutcome::Cancelled,
            ))
        }
    };
    let _ = events.send(crate::agent::AgentEvent::PermissionResolved {
        request_id,
        decision: decision.clone(),
    });
    let allowed = matches!(
        decision,
        crate::permission::PermissionDecision::Allow { .. }
    );
    let outcome = permission_outcome(&request.options, allowed);
    Ok(RequestPermissionResponse::new(outcome))
}

/// 将 Sai 的允许或拒绝决定映射到 agent 提供的权限选项。
///
/// 参数:
/// - `options`: agent 声明的可选权限决定
/// - `allowed`: 用户是否允许本次操作
///
/// 返回:
/// - 匹配到选项时返回 selected，否则返回 cancelled
fn permission_outcome(options: &[PermissionOption], allowed: bool) -> RequestPermissionOutcome {
    let option = options.iter().find(|option| {
        matches!(
            (allowed, option.kind),
            (
                true,
                PermissionOptionKind::AllowOnce | PermissionOptionKind::AllowAlways
            ) | (
                false,
                PermissionOptionKind::RejectOnce | PermissionOptionKind::RejectAlways
            )
        )
    });
    match option {
        Some(option) => RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
            option.option_id.clone(),
        )),
        None => RequestPermissionOutcome::Cancelled,
    }
}

/// 读取文本文件。
///
/// 参数:
/// - `params`: `fs/read_text_file` 参数
///
/// 返回:
/// - 文件内容
async fn read_text_file(
    params: &Value,
    governance: &AcpGovernance,
    events: &EventSender,
) -> Result<String> {
    let request: ReadTextFileRequest =
        super::sdk::from_value(params.clone(), "fs/read_text_file request")?;
    governance.authorize_read(&request.path, events).await?;
    let content = std::fs::read_to_string(&request.path)?;
    // 按需截取行区间：协议的行号从 1 开始
    let line = request.line.map(u64::from);
    let limit = request.limit.map(u64::from);
    if line.is_none() && limit.is_none() {
        return Ok(content);
    }
    let start = line.unwrap_or(1).saturating_sub(1) as usize;
    let lines = content.lines().skip(start);
    Ok(match limit {
        Some(limit) => lines.take(limit as usize).collect::<Vec<_>>().join("\n"),
        None => lines.collect::<Vec<_>>().join("\n"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reads_whole_file_without_range() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        std::fs::write(&path, "one\ntwo\nthree\n").unwrap();

        let governance = AcpGovernance::new(
            dir.path().to_path_buf(),
            None,
            crate::config::AppConfig::default(),
            "session".to_string(),
            None,
            None,
        );
        let (events, _received) = tokio::sync::mpsc::unbounded_channel();
        let content = read_text_file(
            &json!({ "sessionId": "session", "path": path.display().to_string() }),
            &governance,
            &events,
        )
        .await
        .unwrap();

        assert_eq!(content, "one\ntwo\nthree\n");
    }

    /// 协议的行号从 1 开始，取第 2 行起两行应得到 two 与 three。
    #[tokio::test]
    async fn reads_requested_line_range() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        std::fs::write(&path, "one\ntwo\nthree\nfour\n").unwrap();

        let governance = AcpGovernance::new(
            dir.path().to_path_buf(),
            None,
            crate::config::AppConfig::default(),
            "session".to_string(),
            None,
            None,
        );
        let (events, _received) = tokio::sync::mpsc::unbounded_channel();
        let content = read_text_file(
            &json!({
                "sessionId": "session",
                "path": path.display().to_string(),
                "line": 2,
                "limit": 2
            }),
            &governance,
            &events,
        )
        .await
        .unwrap();

        assert_eq!(content, "two\nthree");
    }

    #[tokio::test]
    async fn rejects_request_without_path() {
        let governance = AcpGovernance::new(
            std::env::temp_dir(),
            None,
            crate::config::AppConfig::default(),
            "session".to_string(),
            None,
            None,
        );
        let (events, _received) = tokio::sync::mpsc::unbounded_channel();
        assert!(read_text_file(&json!({}), &governance, &events)
            .await
            .is_err());
    }

    /// 权限决定必须返回 agent 提供的 optionId，不能使用自造状态字符串。
    #[test]
    fn maps_permission_decision_to_advertised_option() {
        let options = vec![
            PermissionOption::new("deny", "Deny", PermissionOptionKind::RejectOnce),
            PermissionOption::new("allow", "Allow", PermissionOptionKind::AllowOnce),
        ];
        let allowed = crate::acp::sdk::to_value(&RequestPermissionResponse::new(
            permission_outcome(&options, true),
        ))
        .unwrap();
        let denied = crate::acp::sdk::to_value(&RequestPermissionResponse::new(
            permission_outcome(&options, false),
        ))
        .unwrap();

        assert_eq!(allowed["outcome"]["outcome"], "selected");
        assert_eq!(allowed["outcome"]["optionId"], "allow");
        assert_eq!(denied["outcome"]["optionId"], "deny");
    }

    /// 构造带审计的治理句柄，模拟非 YOLO 会话。
    ///
    /// 参数:
    /// - `workspace`: 工作区根目录
    /// - `session_id`: 会话标识
    ///
    /// 返回:
    /// - 治理句柄
    fn audited_governance(workspace: &std::path::Path, session_id: &str) -> AcpGovernance {
        let profile = crate::permission::PermissionProfile::new(
            crate::permission::PermissionProfileMode::Audited,
            workspace.to_path_buf(),
            None,
        );
        AcpGovernance::new(
            workspace.to_path_buf(),
            Some(profile),
            crate::config::AppConfig::default(),
            session_id.to_string(),
            None,
            None,
        )
    }

    /// 核心保证：审计模式下外部内核写文件必须弹出权限卡等待人工决定。
    ///
    /// 这条针对的缺陷是「工具调用无法被审核」——此前只做静态校验，
    /// 找不到批准记录就直接失败，用户根本看不到审核卡。
    #[tokio::test]
    async fn write_raises_a_permission_card_and_waits() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().canonicalize().unwrap();
        let target = workspace.join("notes.md");
        let governance = audited_governance(&workspace, "audit-session");
        let (events, mut received) = tokio::sync::mpsc::unbounded_channel();

        let write = tokio::spawn({
            let params = json!({ "sessionId": "session", "path": target.display().to_string(), "content": "hello" });
            let governance = governance.clone();
            async move { write_text_file(&params, &governance, &events).await }
        });

        // 1. 界面先收到权限卡
        let requested = received
            .recv()
            .await
            .expect("a permission card must be raised");
        let request_id = match requested {
            crate::agent::AgentEvent::PermissionRequested(request) => {
                assert_eq!(request.session_id, "audit-session");
                assert_eq!(request.tool, "write_file");
                request.id
            }
            other => panic!("expected a permission request, got {other:?}"),
        };

        // 2. 人工批准后写入才真正发生
        crate::permission::decide_permission(
            &request_id,
            crate::permission::PermissionDecision::allow_once(),
        )
        .unwrap();
        write.await.unwrap().unwrap();

        assert_eq!(std::fs::read_to_string(&target).unwrap(), "hello");
    }

    /// 拒绝时不落盘，并把拒绝原因回给 agent。
    #[tokio::test]
    async fn denied_write_never_touches_the_disk() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().canonicalize().unwrap();
        let target = workspace.join("blocked.md");
        let governance = audited_governance(&workspace, "deny-session");
        let (events, mut received) = tokio::sync::mpsc::unbounded_channel();

        let write = tokio::spawn({
            let params = json!({ "sessionId": "session", "path": target.display().to_string(), "content": "nope" });
            let governance = governance.clone();
            async move { write_text_file(&params, &governance, &events).await }
        });

        let request_id = match received.recv().await.unwrap() {
            crate::agent::AgentEvent::PermissionRequested(request) => request.id,
            other => panic!("expected a permission request, got {other:?}"),
        };
        crate::permission::decide_permission(
            &request_id,
            crate::permission::PermissionDecision::Deny {
                reply: Some("不要动这个文件".to_string()),
            },
        )
        .unwrap();

        let error = write.await.unwrap().expect_err("denied write must fail");
        assert!(format!("{error:#}").contains("不要动这个文件"));
        assert!(!target.exists(), "被拒的写入不应留下文件");
    }

    /// 自动审核抢答后无需人工介入，写入照常发生。
    ///
    /// 针对的缺口是：auto_audit 模式下 ACP 路径此前只走人工审核，
    /// 用户配了自动审核却一直卡着等人点。
    #[tokio::test]
    async fn auto_audit_decision_unblocks_the_write() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().canonicalize().unwrap();
        let target = workspace.join("auto.md");
        let governance = audited_governance(&workspace, "auto-session");
        let (events, mut received) = tokio::sync::mpsc::unbounded_channel();

        let write = tokio::spawn({
            let params = json!({ "sessionId": "session", "path": target.display().to_string(), "content": "auto" });
            let governance = governance.clone();
            async move { write_text_file(&params, &governance, &events).await }
        });

        let request_id = match received.recv().await.unwrap() {
            crate::agent::AgentEvent::PermissionRequested(request) => request.id,
            other => panic!("expected a permission request, got {other:?}"),
        };
        // 模拟审核模型抢先给出放行结论
        crate::permission::decide_permission(
            &request_id,
            crate::permission::PermissionDecision::auto_allow_once(Some(
                "工作区内的常规写入".to_string(),
            )),
        )
        .unwrap();
        write.await.unwrap().unwrap();

        // 决定事件要标出自动来源，界面才能与人工批准区分
        let resolved = received.recv().await.unwrap();
        match resolved {
            crate::agent::AgentEvent::PermissionResolved { decision, .. } => {
                assert!(decision.is_auto_allow(), "决定应标记为自动审核放行");
                assert_eq!(decision.detail(), Some("工作区内的常规写入"));
            }
            other => panic!("expected a resolution, got {other:?}"),
        }
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "auto");
    }

    /// YOLO 会话不绑定权限配置，此时不额外设限，与自带工具行为一致。
    #[tokio::test]
    async fn yolo_session_does_not_add_restrictions() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("free.txt");
        let governance = AcpGovernance::new(
            dir.path().to_path_buf(),
            None,
            crate::config::AppConfig::default(),
            "yolo-session".to_string(),
            None,
            None,
        );
        let (events, _received) = tokio::sync::mpsc::unbounded_channel();

        write_text_file(
            &json!({ "sessionId": "session", "path": target.display().to_string(), "content": "ok" }),
            &governance,
            &events,
        )
        .await
        .unwrap();

        assert!(target.exists());
    }
}
