//! 针对真实 ACP 适配器的联通测试。
//!
//! 默认不跑：需要网络下载适配器，且 Codex 侧要有可用凭据。
//! 设置 `SAI_ACP_LIVE_TEST=1` 后执行 `cargo test acp_live -- --ignored --nocapture` 启用。

use super::protocol::{self, Incoming};
use super::transport::AcpTransport;
use std::collections::BTreeMap;
use std::time::Duration;

/// 判断是否启用联通测试。
///
/// 返回:
/// - 环境变量置位时为 true
fn live_enabled() -> bool {
    std::env::var("SAI_ACP_LIVE_TEST").is_ok_and(|value| value == "1")
}

/// 与真实适配器完成一次 initialize 握手。
///
/// 这条用例锁住两件实测结论：适配器可经 npx 拉起，
/// 且 `protocolVersion` 必须是数字——传字符串会被回 -32602。
#[tokio::test]
#[ignore = "requires network access to download the ACP adapter"]
async fn acp_live_initialize_handshake() {
    if !live_enabled() {
        return;
    }
    let config: crate::config::AgentEngineConfig =
        serde_json::from_str(r#"{"engine":"codex"}"#).unwrap();
    let (program, args) = config.resolved_command().expect("codex preset command");
    let cwd = std::env::temp_dir();
    let (transport, _peer) = AcpTransport::spawn("codex", &program, &args, &BTreeMap::new(), &cwd)
        .await
        .expect("failed to launch the ACP adapter");

    let response = tokio::time::timeout(
        Duration::from_secs(120),
        transport.request(
            "initialize",
            serde_json::json!({
                "protocolVersion": protocol::PROTOCOL_VERSION,
                "clientInfo": { "name": "sai", "version": "test" },
                "clientCapabilities": {
                    "fs": { "readTextFile": true, "writeTextFile": true },
                    "terminal": true
                }
            }),
        ),
    )
    .await
    .expect("initialize timed out")
    .expect("initialize failed");

    assert_eq!(response["protocolVersion"], protocol::PROTOCOL_VERSION);
    assert!(response["agentInfo"]["name"].is_string());
    assert_eq!(
        response["_meta"]["_sai"]["capabilities"]["context_compaction"],
        true
    );
    assert_eq!(response["_meta"]["_sai"]["capabilities"]["memory"], true);
    assert_eq!(
        response["_meta"]["_sai"]["capabilities"]["goal_continuation"],
        true
    );
    assert_eq!(response["_meta"]["_sai"]["capabilities"]["subagents"], true);
    assert_eq!(
        response["_meta"]["_sai"]["native_equivalents"]["context_compaction"],
        "codex"
    );
    assert_eq!(
        response["_meta"]["_sai"]["native_equivalents"]["subagents"],
        "codex"
    );
}

/// 端到端验证：连接真实适配器后能取到 agentInfo。
///
/// 这正是界面上「已连接 Codex 1.1.7」那行的数据来源——
/// 只有真的拉起子进程并完成握手才拿得到，因此可以用来分辨
/// 本轮到底是外部内核在跑，还是 sai 自己的循环。
#[tokio::test]
#[ignore = "requires network access to download the ACP adapter"]
async fn acp_live_reports_agent_info() {
    if !live_enabled() {
        return;
    }
    let config: crate::config::AgentEngineConfig =
        serde_json::from_str(r#"{"engine":"codex"}"#).unwrap();
    let (program, args) = config
        .resolved_command()
        .expect("codex has a preset command");
    let governance = crate::acp::AcpGovernance::new(
        std::env::temp_dir(),
        None,
        crate::config::AppConfig::default(),
        "live-test".to_string(),
        None,
        None,
    );

    let engine = super::AcpEngine::connect(
        "codex",
        &program,
        &args,
        &BTreeMap::new(),
        &std::env::temp_dir(),
        Duration::from_secs(120),
        governance,
    )
    .await
    .expect("failed to connect to the ACP agent");

    let (name, version) = engine.agent_info().expect("agentInfo must be captured");
    println!("connected to {name} {version}");
    assert!(!name.is_empty());
    assert_ne!(version, "-", "adapter should report a real version");
}

/// 真实跑一轮对话：连接 → 建会话 → 发提示 → 收事件。
///
/// 这是唯一能暴露「协议实现对不对」的测试；握手通过不代表对话能跑。
/// 需要本机已有适配器凭据（Claude 侧 `claude /login`）。
#[tokio::test]
#[ignore = "requires adapter credentials and network access"]
async fn acp_live_runs_a_real_turn() {
    if !live_enabled() {
        return;
    }
    let config: crate::config::AgentEngineConfig =
        serde_json::from_str(r#"{"engine":"claude_code"}"#).unwrap();
    let (program, args) = config.resolved_command().expect("preset command");
    let workspace = std::env::temp_dir().join("sai-acp-live");
    std::fs::create_dir_all(&workspace).unwrap();
    let governance = crate::acp::AcpGovernance::new(
        workspace.clone(),
        None,
        crate::config::AppConfig::default(),
        "live-turn".to_string(),
        None,
        None,
    );

    let mut engine = super::AcpEngine::connect(
        "claude_code",
        &program,
        &args,
        &BTreeMap::new(),
        &workspace,
        Duration::from_secs(180),
        governance,
    )
    .await
    .expect("connect failed");

    let (events, mut received) = tokio::sync::mpsc::unbounded_channel();
    let request = crate::agent_engine::TurnRequest {
        input: "Reply with exactly: PONG".to_string(),
        image_urls: Vec::new(),
        cwd: workspace,
        contexts: Vec::new(),
    };

    let result = tokio::time::timeout(
        Duration::from_secs(180),
        <super::AcpEngine as crate::agent_engine::ExternalTurnEngine>::run_turn(
            &mut engine,
            request,
            events,
        ),
    )
    .await
    .expect("turn timed out")
    .expect("turn failed");

    let mut kinds = Vec::new();
    while let Ok(event) = received.try_recv() {
        kinds.push(format!("{event:?}").chars().take(80).collect::<String>());
    }
    println!("--- events ---");
    for kind in &kinds {
        println!("{kind}");
    }
    println!("--- content ---\n{}", result.content);
    assert!(
        !result.content.trim().is_empty(),
        "assistant reply is empty"
    );
}

/// 真实验证 Codex 会话恢复不会把历史正文计入新一轮结果。
///
/// 该场景要求销毁并重新连接适配器，因为 Web 每轮都会重新构造 Agent，
/// `session/load` 的历史重放只会在第二次连接时出现。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[tokio::test]
#[ignore = "requires Codex credentials and network access"]
async fn acp_live_codex_resume_does_not_replay_history() {
    if !live_enabled() {
        return;
    }
    let config: crate::config::AgentEngineConfig =
        serde_json::from_str(r#"{"engine":"codex"}"#).unwrap();
    let (program, args) = config.resolved_command().expect("preset command");
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path().join("workspace");
    let state_dir = temp.path().join("state");
    std::fs::create_dir_all(&workspace).unwrap();
    let governance = crate::acp::AcpGovernance::new(
        workspace.clone(),
        None,
        crate::config::AppConfig::default(),
        "live-codex-resume".to_string(),
        None,
        Some(&state_dir),
    );

    let first = run_codex_resume_turn(
        &program,
        &args,
        &workspace,
        governance.clone(),
        "Reply with exactly: FIRST_RESUME_MARKER",
    )
    .await;
    assert!(first.contains("FIRST_RESUME_MARKER"));

    let second = run_codex_resume_turn(
        &program,
        &args,
        &workspace,
        governance,
        "Reply with exactly: SECOND_RESUME_MARKER",
    )
    .await;
    assert!(second.contains("SECOND_RESUME_MARKER"));
    assert!(
        !second.contains("FIRST_RESUME_MARKER"),
        "restored history leaked into the new turn: {second}"
    );
}

/// 启动一次 Codex ACP 连接并执行单轮提示。
///
/// 参数:
/// - `program`: ACP 启动程序
/// - `args`: ACP 启动参数
/// - `workspace`: 临时工作区
/// - `governance`: 带持久化会话标识的治理句柄
/// - `input`: 本轮提示
///
/// 返回:
/// - 本轮聚合正文
async fn run_codex_resume_turn(
    program: &str,
    args: &[String],
    workspace: &std::path::Path,
    governance: crate::acp::AcpGovernance,
    input: &str,
) -> String {
    let mut engine = super::AcpEngine::connect(
        "codex",
        program,
        args,
        &BTreeMap::new(),
        workspace,
        Duration::from_secs(180),
        governance,
    )
    .await
    .expect("connect failed");
    let (events, _received) = tokio::sync::mpsc::unbounded_channel();
    let request = crate::agent_engine::TurnRequest {
        input: input.to_string(),
        image_urls: Vec::new(),
        cwd: workspace.to_path_buf(),
        contexts: Vec::new(),
    };
    tokio::time::timeout(
        Duration::from_secs(180),
        <super::AcpEngine as crate::agent_engine::ExternalTurnEngine>::run_turn(
            &mut engine,
            request,
            events,
        ),
    )
    .await
    .expect("turn timed out")
    .expect("turn failed")
    .content
}

/// 验证解析器能吃下适配器实际回出的报文。
///
/// 样本取自本机 codex-acp 1.1.7 与 claude-code-acp 0.16.2 的真实响应，
/// 因此不需要网络也能跑。
#[test]
fn parses_real_adapter_responses() {
    let codex = r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1,"agentInfo":{"name":"@agentclientprotocol/codex-acp","title":"Codex","version":"1.1.7"},"agentCapabilities":{"loadSession":true,"promptCapabilities":{"embeddedContext":true,"image":true}},"authMethods":[{"id":"api-key","name":"API Key"}],"_meta":{"steering":{"supported":true}}}}"#;
    match protocol::parse_incoming(codex) {
        Some(Incoming::Response { result, .. }) => {
            let value = result.expect("codex handshake should succeed");
            assert_eq!(value["protocolVersion"], 1);
            assert_eq!(value["agentInfo"]["title"], "Codex");
            assert_eq!(value["agentCapabilities"]["loadSession"], true);
        }
        other => panic!("expected a response, got {other:?}"),
    }

    let claude = r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1,"agentCapabilities":{"promptCapabilities":{"image":true,"embeddedContext":true},"mcpCapabilities":{"http":true,"sse":true},"loadSession":true},"agentInfo":{"name":"@zed-industries/claude-code-acp","title":"Claude Code","version":"0.16.2"},"authMethods":[{"description":"Run `claude /login` in the terminal","name":"Log in with Claude Code","id":"claude-login"}]}}"#;
    match protocol::parse_incoming(claude) {
        Some(Incoming::Response { result, .. }) => {
            let value = result.expect("claude handshake should succeed");
            assert_eq!(value["agentInfo"]["title"], "Claude Code");
            assert_eq!(value["authMethods"][0]["id"], "claude-login");
        }
        other => panic!("expected a response, got {other:?}"),
    }
}

/// 验证版本类型错误会被识别为错误响应。
///
/// 传字符串版本号时适配器的真实回包，用来固化「必须传数字」这条结论。
#[test]
fn parses_invalid_protocol_version_error() {
    let line = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32602,"message":"Invalid params","data":{"protocolVersion":{"_errors":["Invalid input: expected number, received string"]}}}}"#;
    match protocol::parse_incoming(line) {
        Some(Incoming::Response { result, .. }) => {
            let error = result.expect_err("string protocol version must be rejected");
            assert_eq!(error.code, -32602);
        }
        other => panic!("expected an error response, got {other:?}"),
    }
}
