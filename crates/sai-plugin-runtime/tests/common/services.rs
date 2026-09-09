use anyhow::{bail, Result};
use async_trait::async_trait;
use sai_plugin_runtime::host::{HostTool, InvocationServices, ModelRequest, ModelResponse};
use sai_plugin_runtime::{Capabilities, InvocationContext, PluginRuntime, ToolAccess};
use serde_json::json;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

#[derive(Default)]
pub struct RecordingServices {
    pub models: Mutex<Vec<ModelRequest>>,
    pub calls: Mutex<Vec<(String, String)>>,
    pub responses: Mutex<VecDeque<ModelResponse>>,
    pub block_model: AtomicBool,
    pub block_tool: AtomicBool,
    pub active: AtomicUsize,
    pub entered: Notify,
    pub released: Notify,
}

struct Flight<'a>(&'a RecordingServices);

impl Drop for Flight<'_> {
    /// 【插件测试】【请求释放】证明取消已经回收实际宿主 Future。
    /// @returns 无
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
        self.0.released.notify_one();
    }
}

impl RecordingServices {
    /// 【插件测试】【请求标记】记录真实宿主操作开始，并在退出时减少在途计数。
    /// @returns 当前请求的释放守卫
    fn enter(&self) -> Flight<'_> {
        self.active.fetch_add(1, Ordering::SeqCst);
        self.entered.notify_one();
        Flight(self)
    }
}

#[async_trait]
impl InvocationServices for RecordingServices {
    /// 【插件测试】【工具目录】提供读写工具，供运行时进一步收窄。
    /// @returns 三个可观察的测试工具
    fn tools(&self) -> Result<Vec<HostTool>> {
        Ok([
            ("read", ToolAccess::ReadOnly),
            ("write", ToolAccess::Writes),
            ("undeclared", ToolAccess::ReadOnly),
        ]
        .into_iter()
        .map(|(name, access)| HostTool {
            name: name.into(),
            display_name: name.into(),
            description: name.into(),
            parameters: json!({"type":"object"}),
            access,
        })
        .collect())
    }

    /// 【插件测试】【工具调用】记录参数，并可暂停以验证取消。
    /// @param name 工具名；arguments 为原始 JSON 参数
    /// @returns 固定文本结果
    async fn call_tool(&self, name: &str, arguments: &str) -> Result<String> {
        let _flight = self.enter();
        self.calls
            .lock()
            .unwrap()
            .push((name.into(), arguments.into()));
        if self.block_tool.load(Ordering::SeqCst) {
            std::future::pending::<()>().await;
        }
        if name == "undeclared" {
            bail!("undeclared tool reached host");
        }
        Ok("tool result".into())
    }

    /// 【插件测试】【模型调用】保留完整请求，使用样本响应或暂停等待取消。
    /// @param request 模型请求；max_bytes 由运行时在返回后校验
    /// @returns 固定或排队的响应
    async fn complete(&self, request: ModelRequest, _max_bytes: usize) -> Result<ModelResponse> {
        let _flight = self.enter();
        self.models.lock().unwrap().push(request);
        if self.block_model.load(Ordering::SeqCst) {
            std::future::pending::<()>().await;
        }
        Ok(self
            .responses
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| ModelResponse {
                content: "model result".into(),
                ..Default::default()
            }))
    }
}

/// 【插件测试】【服务运行时】在真实 Lua 运行时加载声明和授权不同的脚本。
/// @param source 脚本；declared 为清单声明；granted 为实际授权；change 可调整资源限制
/// @returns 可独立执行的插件
pub fn runtime(
    source: &str,
    declared: Capabilities,
    granted: Capabilities,
    change: impl FnOnce(&mut sai_plugin_runtime::ExecutionLimits),
) -> PluginRuntime {
    let mut package = super::package(source);
    package.manifest.capabilities = declared;
    change(&mut package.manifest.limits);
    PluginRuntime::load(
        package,
        json!({}),
        granted,
        Arc::new(super::RecordingHost::default()),
    )
    .unwrap()
}

/// 【插件测试】【服务上下文】绑定观察宿主，避免调用任何真实模型或工具。
/// @param services 观察宿主；allow_writes 为宿主确认的写入权限
/// @returns 本次调用上下文
pub fn context(services: Arc<RecordingServices>, allow_writes: bool) -> InvocationContext {
    InvocationContext {
        services: Some(services),
        allow_writes,
        ..Default::default()
    }
}
