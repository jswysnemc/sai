//! 独立、可取消的 MCP 连接：保留结构化结果与图片，不进入共享外部工具连接池。

use super::{McpServerConfig, PooledClient};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

/// 单一原生会话持有的连接，连接失败或请求取消后不复用半读状态。
pub(crate) struct McpConnection {
    config: McpServerConfig,
    client: Arc<Mutex<Option<PooledClient>>>,
}

impl McpConnection {
    /// 【原生桥接】【连接构造】声明连接配置，首次调用前不启动子进程。
    /// @param config 为可信宿主提供的 MCP 配置
    /// @returns 延迟连接实例
    pub(crate) fn new(config: McpServerConfig) -> Self {
        Self {
            config,
            client: Arc::new(Mutex::new(None)),
        }
    }

    /// 【原生桥接】【工具调用】在常驻运行时串行发送一次请求，不重放失败调用。
    /// @param name 为原生工具名称；arguments 为已校验参数
    /// @returns 原始 MCP result，保留 structuredContent、图片和 isError
    pub(crate) async fn call(&self, name: &str, arguments: Value) -> Result<Value> {
        let config = self.config.clone();
        let slot = self.client.clone();
        let name = name.to_owned();
        let task = super::runtime::mcp_runtime().spawn(async move {
            let timeout = Duration::from_millis(config.timeout_ms.unwrap_or(30_000));
            tokio::time::timeout(timeout, async move {
                let mut slot = slot.lock().await;
                // 1. 【原生桥接】【连接所有权】请求期间移出连接，取消会销毁进程而非留下半条响应
                let mut client = match slot.take() {
                    Some(client) => client,
                    None => PooledClient::connect(&config).await?,
                };
                let result = client
                    .request(
                        "tools/call",
                        json!({"name":name, "arguments":arguments}),
                        config.timeout_ms,
                    )
                    .await?;
                // 2. 【原生桥接】【复用边界】仅完整收到响应时归还连接，超时不自动重试
                *slot = Some(client);
                Ok::<_, anyhow::Error>(result)
            })
            .await
            .context("Computer Use request timed out; its outcome may be unknown")?
        });
        let _abort = AbortOnDrop(task.abort_handle());
        task.await.context("Computer Use connection task failed")?
    }

    /// 【原生桥接】【会话释放】关闭当前连接，下一次显式调用才重新初始化。
    /// @returns 无
    pub(crate) async fn close(&self) {
        self.client.lock().await.take();
    }
}

/// 调用方取消等待时同步取消原生任务，避免后台继续发送输入。
struct AbortOnDrop(tokio::task::AbortHandle);

impl Drop for AbortOnDrop {
    /// 【原生桥接】【取消传播】取消尚未结束的请求，已完成任务不受影响。
    /// @returns 无
    fn drop(&mut self) {
        self.0.abort();
    }
}
