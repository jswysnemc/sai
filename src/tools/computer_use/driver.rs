//! 将已校验的桌面动作交给常驻 cua-driver MCP 会话。

use crate::config::{ComputerUseConfig, McpServerConfig};
use crate::mcp::McpConnection;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

/// 原生驱动边界，测试使用内存实现或本地协议夹具。
#[async_trait]
pub(super) trait NativeDriver: Send + Sync {
    /// 【桌面操作】【原生调用】发送一次确定的动作，不隐式重试。
    /// @param name 为原生工具名；args 为参数
    /// @returns 完整 MCP result
    async fn call(&self, name: &str, args: Value) -> Result<Value>;

    /// 【桌面操作】【驱动关闭】释放当前会话和原生快照，不停止其他客户端的服务。
    /// @returns 无
    async fn close(&self);
}

/// 使用独立连接而非外部 MCP 工具池，保证取消与快照生命周期一致。
pub(super) struct CuaDriver {
    connection: McpConnection,
}

impl CuaDriver {
    /// 【桌面操作】【驱动构造】只记录本地可执行文件，首次操作时才连接。
    /// @param config 为用户桌面配置
    /// @returns 不触发桌面或权限弹窗的驱动实例
    pub(super) fn new(config: &ComputerUseConfig) -> Self {
        Self::with_connection(McpConnection::new(McpServerConfig {
            id: "sai-computer-use".into(),
            enabled: true,
            transport: "stdio".into(),
            command: config.driver_path.clone(),
            args: vec!["mcp".into()],
            env: HashMap::new(),
            cwd: None,
            url: None,
            message_url: None,
            headers: HashMap::new(),
            timeout_ms: Some(config.timeout_ms),
        }))
    }

    /// 【桌面操作】【传输组合】复用既有 MCP 协议，同时允许测试注入隔离连接。
    /// @param connection 为当前原生会话独占的连接
    /// @returns 驱动实例
    pub(super) fn with_connection(connection: McpConnection) -> Self {
        Self { connection }
    }
}

#[async_trait]
impl NativeDriver for CuaDriver {
    /// 【桌面操作】【驱动请求】保持原始图片和错误标志，不把失败包装成成功文本。
    /// @param name 为原生工具名；args 为已校验参数
    /// @returns 驱动原始结果；传输失败时说明禁止盲目重试
    async fn call(&self, name: &str, args: Value) -> Result<Value> {
        self.connection.call(name, args).await.with_context(|| {
            format!("Computer Use driver call {name} failed. Install/configure cua-driver and grant its OS permissions. Do not retry uncertain input; get_app_state again first.")
        })
    }

    /// 【桌面操作】【驱动释放】关闭本客户端连接，保留用户的全局驱动设置。
    /// @returns 无
    async fn close(&self) {
        self.connection.close().await;
    }
}
