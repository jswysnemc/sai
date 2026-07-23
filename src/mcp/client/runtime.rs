use super::{McpRuntimeStatus, McpServerConfig, McpToolInfo, PooledClient};
use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;

static POOL: OnceLock<Mutex<HashMap<String, Arc<Mutex<PooledClient>>>>> = OnceLock::new();
/// 进程级 MCP runtime：stdio Child 与连接池必须活在同一 runtime 上。
static MCP_RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// 返回进程级 MCP 客户端连接池。
///
/// 返回:
/// - 以服务器 ID 为键的共享客户端连接池
fn pool() -> &'static Mutex<HashMap<String, Arc<Mutex<PooledClient>>>> {
    POOL.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 返回常驻的 MCP 多线程运行时。
///
/// 返回:
/// - 承载 stdio 子进程和网络连接的进程级运行时
fn mcp_runtime() -> &'static tokio::runtime::Runtime {
    MCP_RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .thread_name("sai-mcp")
            .build()
            .expect("failed to create MCP runtime")
    })
}

/// 在 MCP 专用运行时上同步执行 future。
///
/// 参数:
/// - `future`: 需要在 MCP 运行时执行的异步任务
///
/// 返回:
/// - 异步任务的输出
pub fn block_on_mcp<F>(future: F) -> F::Output
where
    F: std::future::Future,
{
    mcp_runtime().block_on(future)
}

/// 从任意异步上下文把 MCP 工作调度到专用运行时。
///
/// 参数:
/// - `future`: 需要调度的异步任务
///
/// 返回:
/// - 任务输出；调度失败时返回错误
pub async fn run_on_mcp_runtime<F, T>(future: F) -> Result<T>
where
    F: std::future::Future<Output = Result<T>> + Send + 'static,
    T: Send + 'static,
{
    let handle = mcp_runtime().handle().clone();
    tokio::task::spawn_blocking(move || handle.block_on(future))
        .await
        .map_err(|error| anyhow::anyhow!("mcp runtime worker failed: {error}"))?
}

/// 计算影响 MCP 连接复用的稳定配置指纹。
///
/// 参数:
/// - `config`: MCP 服务器配置
///
/// 返回:
/// - 用于识别连接配置变化的指纹文本
pub(super) fn fingerprint(config: &McpServerConfig) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}|{}|{}",
        config.id,
        config.transport,
        config.command,
        config.args.join("\u{1f}"),
        config.url.clone().unwrap_or_default(),
        config.message_url.clone().unwrap_or_default(),
        config.cwd.clone().unwrap_or_default(),
        config.timeout_ms.unwrap_or(0)
    )
}

/// 获取或重建指定服务器的池化客户端并执行操作。
///
/// 参数:
/// - `config`: MCP 服务器配置
/// - `f`: 使用池化客户端执行的异步操作
///
/// 返回:
/// - 客户端操作结果
async fn with_client<T, F, Fut>(config: &McpServerConfig, f: F) -> Result<T>
where
    F: FnOnce(Arc<Mutex<PooledClient>>) -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let key = config.id.clone();
    let fingerprint = fingerprint(config);
    let client = {
        let mut map = pool().lock().await;
        if let Some(existing) = map.get(&key) {
            let guard = existing.lock().await;
            if guard.config_fingerprint == fingerprint && guard.initialized {
                drop(guard);
                existing.clone()
            } else {
                drop(guard);
                map.remove(&key);
                let created = Arc::new(Mutex::new(PooledClient::connect(config).await?));
                map.insert(key.clone(), created.clone());
                created
            }
        } else {
            let created = Arc::new(Mutex::new(PooledClient::connect(config).await?));
            map.insert(key, created.clone());
            created
        }
    };
    f(client).await
}

/// 停止连接池中的指定服务器。
///
/// 参数:
/// - `server_id`: MCP 服务器 ID
///
/// 返回:
/// - 是否移除了活动连接
pub async fn stop_server(server_id: &str) -> bool {
    let server_id = server_id.to_string();
    run_on_mcp_runtime(async move { Ok(stop_server_on_rt(&server_id).await) })
        .await
        .unwrap_or(false)
}

/// 在 MCP 运行时内移除指定服务器连接。
///
/// 参数:
/// - `server_id`: MCP 服务器 ID
///
/// 返回:
/// - 是否移除了活动连接
async fn stop_server_on_rt(server_id: &str) -> bool {
    let mut map = pool().lock().await;
    map.remove(server_id).is_some()
}

/// 清空全部 MCP 连接。
///
/// 返回:
/// - 无
pub async fn stop_all_servers() {
    let _ = run_on_mcp_runtime(async move {
        let mut map = pool().lock().await;
        map.clear();
        Ok::<(), anyhow::Error>(())
    })
    .await;
}

/// 读取指定服务器列表的运行状态。
///
/// 参数:
/// - `servers`: MCP 服务器配置列表
///
/// 返回:
/// - 与输入顺序一致的运行状态
pub async fn runtime_statuses(servers: &[McpServerConfig]) -> Vec<McpRuntimeStatus> {
    let servers = servers.to_vec();
    run_on_mcp_runtime(async move { Ok(runtime_statuses_on_rt(&servers).await) })
        .await
        .unwrap_or_default()
}

/// 在 MCP 运行时内读取服务器状态。
///
/// 参数:
/// - `servers`: MCP 服务器配置列表
///
/// 返回:
/// - 与输入顺序一致的运行状态
async fn runtime_statuses_on_rt(servers: &[McpServerConfig]) -> Vec<McpRuntimeStatus> {
    let map = pool().lock().await;
    servers
        .iter()
        .map(|server| {
            if let Some(client) = map.get(&server.id) {
                // 1. 非阻塞读取客户端状态，避免与正在执行的请求互相等待
                if let Ok(guard) = client.try_lock() {
                    return McpRuntimeStatus {
                        server_id: server.id.clone(),
                        transport: server.transport.clone(),
                        running: true,
                        initialized: guard.initialized,
                        last_error: guard.last_error.clone(),
                    };
                }
                return McpRuntimeStatus {
                    server_id: server.id.clone(),
                    transport: server.transport.clone(),
                    running: true,
                    initialized: true,
                    last_error: None,
                };
            }
            McpRuntimeStatus {
                server_id: server.id.clone(),
                transport: server.transport.clone(),
                running: false,
                initialized: false,
                last_error: None,
            }
        })
        .collect()
}

/// 列出单个 MCP 服务器提供的工具。
///
/// 参数:
/// - `config`: MCP 服务器配置
///
/// 返回:
/// - 服务器工具定义列表
pub async fn list_server_tools(config: &McpServerConfig) -> Result<Vec<McpToolInfo>> {
    let config = config.clone();
    run_on_mcp_runtime(async move { list_server_tools_on_rt(&config).await }).await
}

/// 在 MCP 运行时内列出服务器工具。
///
/// 参数:
/// - `config`: MCP 服务器配置
///
/// 返回:
/// - 服务器工具定义列表
pub(in crate::mcp) async fn list_server_tools_on_rt(
    config: &McpServerConfig,
) -> Result<Vec<McpToolInfo>> {
    with_client(config, |client| async move {
        let mut guard = client.lock().await;
        let tools = guard.list_tools(config.timeout_ms).await?;
        Ok(tools
            .into_iter()
            .map(|(name, description, input_schema)| McpToolInfo {
                server_id: config.id.clone(),
                name,
                description,
                input_schema,
            })
            .collect())
    })
    .await
}

/// 调用指定 MCP 工具。
///
/// 参数:
/// - `config`: MCP 服务器配置
/// - `tool_name`: 远端工具名称
/// - `arguments`: 工具参数 JSON
///
/// 返回:
/// - 远端工具文本结果
pub async fn call_server_tool(
    config: &McpServerConfig,
    tool_name: &str,
    arguments: Value,
) -> Result<String> {
    let config = config.clone();
    let tool_name = tool_name.to_string();
    run_on_mcp_runtime(async move {
        with_client(&config, |client| async move {
            let mut guard = client.lock().await;
            guard
                .call_tool(&tool_name, arguments, config.timeout_ms)
                .await
        })
        .await
    })
    .await
}

/// 重新连接服务器并返回可用工具摘要。
///
/// 参数:
/// - `config`: MCP 服务器配置
///
/// 返回:
/// - 工具数量和工具名称列表
pub async fn test_server(config: &McpServerConfig) -> Result<(usize, Vec<String>)> {
    let config = config.clone();
    run_on_mcp_runtime(async move {
        // 1. 清理旧连接，确保测试覆盖完整初始化过程
        let _ = stop_server_on_rt(&config.id).await;
        let tools = list_server_tools_on_rt(&config).await?;
        Ok((
            tools.len(),
            tools.into_iter().map(|tool| tool.name).collect(),
        ))
    })
    .await
}
