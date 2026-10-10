//! 经本地 MCP 服务验证真实连接、工具注册、图片交付和取消，不接触原生桌面。

use super::driver::CuaDriver;
use super::session::{ComputerSession, DesktopCoordinator};
use super::test_support::{input_args, observe_args, state};
use crate::config::{ComputerUseConfig, McpServerConfig};
use crate::mcp::McpConnection;
use crate::tools::{ToolOutput, ToolRegistry};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tokio::sync::Notify;

/// 合成服务的请求记录和可控暂停点。
#[derive(Default)]
struct ServerState {
    requests: Mutex<Vec<Value>>,
    block_input: AtomicBool,
    started: Notify,
    release: Notify,
}

/// 生命周期绑定到测试作用域的本地 MCP 服务。
struct Fixture {
    state: Arc<ServerState>,
    config: McpServerConfig,
    task: tokio::task::JoinHandle<()>,
}

impl Fixture {
    /// 【桌面测试】【协议服务】启动仅监听回环地址的 MCP 替身。
    /// @param timeout_ms 为待测客户端的完整调用超时
    /// @returns 自动清理的服务与连接配置
    async fn start(timeout_ms: u64) -> Self {
        let state = Arc::new(ServerState::default());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let router = Router::new()
            .route("/mcp", post(rpc))
            .with_state(state.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Self {
            state,
            config: McpServerConfig {
                id: "computer-use-fixture".into(),
                enabled: true,
                transport: "http".into(),
                command: String::new(),
                args: Vec::new(),
                env: HashMap::new(),
                cwd: None,
                url: Some(format!("http://{address}/mcp")),
                message_url: None,
                headers: HashMap::new(),
                timeout_ms: Some(timeout_ms),
            },
            task,
        }
    }

    /// 【桌面测试】【工具装配】通过真实注册入口连接本地协议替身。
    /// @returns 可调用的原生工具注册表
    fn registry(&self) -> ToolRegistry {
        let connection = McpConnection::new(self.config.clone());
        let session = ComputerSession::new(
            Arc::new(CuaDriver::with_connection(connection)),
            Arc::new(DesktopCoordinator::default()),
        );
        let mut registry = ToolRegistry::new();
        super::register_session(
            &mut registry,
            &ComputerUseConfig {
                enabled: true,
                ..Default::default()
            },
            session,
        );
        registry
    }

    /// 【桌面测试】【协议计数】统计真实发送的指定方法，检查连接复用和是否重放。
    /// @param method 为 JSON-RPC 方法；tool 为可选工具名
    /// @returns 匹配的请求数
    fn count(&self, method: &str, tool: Option<&str>) -> usize {
        self.state
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|request| {
                request["method"] == method
                    && tool.is_none_or(|tool| request["params"]["name"] == tool)
            })
            .count()
    }
}

impl Drop for Fixture {
    /// 【桌面测试】【服务清理】释放暂停请求并关闭监听，不遗留测试服务。
    /// @returns 无
    fn drop(&mut self) {
        self.state.release.notify_waiters();
        self.task.abort();
    }
}

/// 【桌面测试】【协议处理】实现初始化、窗口观察和一次可暂停输入。
/// @param state 为测试状态；request 为 JSON-RPC 请求
/// @returns 协议响应或初始化通知的空响应
async fn rpc(State(server): State<Arc<ServerState>>, Json(request): Json<Value>) -> Response {
    server.requests.lock().unwrap().push(request.clone());
    if request.get("id").is_none() {
        return StatusCode::NO_CONTENT.into_response();
    }
    let result = match request["method"].as_str().unwrap() {
        "initialize" => {
            json!({"protocolVersion":"2025-11-25","capabilities":{"tools":{}},"serverInfo":{"name":"fixture","version":"1"}})
        }
        "tools/call" => match request["params"]["name"].as_str().unwrap() {
            "get_window_state" => state(true),
            "press_key" | "click" => {
                if server.block_input.load(Ordering::SeqCst) {
                    server.started.notify_one();
                    server.release.notified().await;
                }
                json!({"structuredContent":{"effect":"applied"},"content":[{"type":"text","text":"fixture input accepted"}]})
            }
            _ => json!({"isError":true,"content":[{"type":"text","text":"unknown fixture tool"}]}),
        },
        _ => json!({}),
    };
    Json(json!({"jsonrpc":"2.0","id":request["id"],"result":result})).into_response()
}

/// 【桌面测试】【注册表调用】通过真实工具调用边界保留模型图片附件。
/// @param registry 为工具表；args 为公开工具参数
/// @returns 完整工具输出
async fn call(registry: &ToolRegistry, args: Value) -> anyhow::Result<ToolOutput> {
    let (sender, _receiver) = tokio::sync::mpsc::unbounded_channel();
    registry
        .call_with_progress(super::TOOL_NAME, &args.to_string(), sender)
        .await
}

/// 【桌面测试】【协议闭环】初始化仅一次，截图穿过 MCP 和工具注册表后仍完整可见。
#[tokio::test]
async fn registry_mcp_round_trip_keeps_images_and_reuses_the_connection() {
    let fixture = Fixture::start(2000).await;
    let registry = fixture.registry();
    let observed = call(&registry, observe_args()).await.unwrap();
    assert_eq!(observed.model_attachments.len(), 1);
    assert!(observed.content.contains("100x60"));
    let mut click = input_args(&observed.content, "click");
    click["element_index"] = json!("g1:0");
    call(&registry, click).await.unwrap();
    assert_eq!(fixture.count("initialize", None), 1);
    assert_eq!(fixture.count("tools/call", Some("click")), 1);
    call(&registry, json!({"action":"reset"})).await.unwrap();
}

/// 【桌面测试】【取消闭环】取消后快照不可复用，下一次观察重新建连且不重放输入。
#[tokio::test]
async fn cancelling_a_call_discards_connection_and_snapshot() {
    let fixture = Fixture::start(3000).await;
    let registry = fixture.registry();
    let observed = call(&registry, observe_args()).await.unwrap();
    let mut key = input_args(&observed.content, "press_key");
    key["key"] = json!("Return");
    fixture.state.block_input.store(true, Ordering::SeqCst);
    let pending_registry = registry.clone();
    let pending_args = key.clone();
    let pending = tokio::spawn(async move { call(&pending_registry, pending_args).await });
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        fixture.state.started.notified(),
    )
    .await
    .unwrap();
    pending.abort();
    assert!(pending.await.unwrap_err().is_cancelled());
    assert!(call(&registry, key)
        .await
        .unwrap_err()
        .to_string()
        .contains("No current"));
    fixture.state.block_input.store(false, Ordering::SeqCst);
    fixture.state.release.notify_waiters();
    call(&registry, observe_args()).await.unwrap();
    assert_eq!(fixture.count("initialize", None), 2);
    assert_eq!(fixture.count("tools/call", Some("press_key")), 1);
    call(&registry, json!({"action":"reset"})).await.unwrap();
}

/// 【桌面测试】【超时闭环】完整调用超时后不重试，重新观察才恢复可用连接。
#[tokio::test]
async fn timed_out_native_input_is_never_replayed() {
    let fixture = Fixture::start(250).await;
    let registry = fixture.registry();
    let observed = call(&registry, observe_args()).await.unwrap();
    let mut key = input_args(&observed.content, "press_key");
    key["key"] = json!("Tab");
    fixture.state.block_input.store(true, Ordering::SeqCst);
    let error = call(&registry, key.clone()).await.unwrap_err();
    assert!(format!("{error:#}").contains("timed out"), "{error:#}");
    assert!(call(&registry, key).await.is_err());
    fixture.state.block_input.store(false, Ordering::SeqCst);
    fixture.state.release.notify_waiters();
    call(&registry, observe_args()).await.unwrap();
    assert_eq!(fixture.count("initialize", None), 2);
    assert_eq!(fixture.count("tools/call", Some("press_key")), 1);
}
