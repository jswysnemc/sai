use super::support::{descriptor, FixtureHost};
use crate::config::AppConfig;
use crate::llm::OpenAiCompatibleClient;
use crate::paths::SaiPaths;
use crate::plugins::discovery::PluginDescriptor;
use crate::plugins::registry::register_descriptor;
use crate::tools::ToolRegistry;
use axum::body::Body;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// 【插件测试】【模型样本】一次真实 SSE 请求对应的正文、工具建议和可选用量。
pub(super) struct ModelReply {
    pub delta: Value,
    pub usage: Option<Value>,
    complete: bool,
}

impl ModelReply {
    /// 【插件测试】【文本响应】构造固定正文，默认一次请求消耗 20 token。
    /// @param text 模型正文
    /// @returns 用于本地服务的响应样本
    pub fn text(text: &str) -> Self {
        Self::delta(json!({"role":"assistant", "content":text}))
    }

    /// 【插件测试】【增量响应】构造包含工具调用或思考的 SSE 增量。
    /// @param delta 供应商协议增量
    /// @returns 包含固定用量的响应样本
    pub fn delta(delta: Value) -> Self {
        Self {
            delta,
            usage: Some(json!({"prompt_tokens":10, "completion_tokens":10, "total_tokens":20})),
            complete: true,
        }
    }

    /// 【插件测试】【缺失用量】模拟供应商没有返回 usage。
    /// @returns 移除用量后的样本
    pub fn without_usage(mut self) -> Self {
        self.usage = None;
        self
    }

    /// 【插件测试】【未结束响应】首帧后保持连接，用于验证宿主在流式阶段提前取消。
    /// @returns 不发送结束帧的样本
    pub fn hold_open(mut self) -> Self {
        self.complete = false;
        self
    }

    /// 【插件测试】【SSE 编码】将样本编码为真实客户端可解析的两帧响应。
    /// @returns 包含结束标记的事件流正文
    fn encode(self) -> (String, bool) {
        let finish = if self.delta.get("tool_calls").is_some() {
            "tool_calls"
        } else {
            "stop"
        };
        let first = json!({"id":"fixture", "object":"chat.completion.chunk", "model":"fixture",
            "choices":[{"index":0, "delta":self.delta, "finish_reason":null}]});
        if !self.complete {
            return (format!("data: {first}\n\n"), true);
        }
        let mut last = json!({"id":"fixture", "object":"chat.completion.chunk", "model":"fixture",
            "choices":[{"index":0, "delta":{}, "finish_reason":finish}]});
        if let Some(usage) = self.usage {
            last["usage"] = usage;
        }
        (
            format!("data: {first}\n\ndata: {last}\n\ndata: [DONE]\n\n"),
            false,
        )
    }
}

#[derive(Default)]
struct ModelState {
    replies: Mutex<VecDeque<(String, bool)>>,
    requests: Mutex<Vec<Value>>,
}

/// 【插件测试】【模型服务】只监听回环地址，捕获真实客户端请求并按顺序提供样本。
pub(super) struct ModelFixture {
    base_url: String,
    state: Arc<ModelState>,
    task: tokio::task::JoinHandle<()>,
}

impl ModelFixture {
    /// 【插件测试】【模型启动】绑定临时端口并启动服务，测试结束自动释放监听器。
    /// @param replies 按顺序消费的模型响应
    /// @returns 可生成配置并读取请求记录的服务
    pub async fn start(replies: Vec<ModelReply>) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}/v1", listener.local_addr().unwrap());
        let state = Arc::new(ModelState {
            replies: Mutex::new(replies.into_iter().map(ModelReply::encode).collect()),
            ..Default::default()
        });
        let router = Router::new()
            .route("/v1/chat/completions", post(respond))
            .with_state(state.clone());
        let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        Self {
            base_url,
            state,
            task,
        }
    }

    /// 【插件测试】【模型配置】创建仅访问当前本地服务的独立配置。
    /// @param model 请求中应出现的模型标识
    /// @returns 不读取用户供应商或凭据的配置
    pub fn config(&self, model: &str) -> AppConfig {
        serde_json::from_value(json!({
            "active_provider":"fixture",
            "providers":[{"id":"fixture", "display_name":"Fixture", "enabled":true,
                "base_url":self.base_url, "protocol":"auto", "api_key":"fixture-only",
                "models":[model], "default_model":model, "timeout_seconds":5}],
            "notification":{"enabled":false, "sound":false},
            "load_instruction_files":false,
            "skills":{"enabled":false},
            "memory":{"enabled":false}
        }))
        .unwrap()
    }

    /// 【插件测试】【真实客户端】按指定模型构造正式客户端。
    /// @param model 模型标识；paths 为临时应用路径
    /// @returns 连接本地服务的客户端
    pub fn client(&self, model: &str, paths: &SaiPaths) -> OpenAiCompatibleClient {
        OpenAiCompatibleClient::from_config(&self.config(model), paths).unwrap()
    }

    /// 【插件测试】【请求快照】读取所有已收到的请求，不暴露鉴权请求头。
    /// @returns 按接收顺序排列的 JSON 正文
    pub fn requests(&self) -> Vec<Value> {
        self.state.requests.lock().unwrap().clone()
    }
}

impl Drop for ModelFixture {
    /// 【插件测试】【模型回收】结束测试服务，不在后台保留监听任务。
    /// @returns 无
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// 【插件测试】【模型应答】记录请求并返回 SSE，样本耗尽时明确失败。
/// @param state 本地服务状态；request 为正式客户端发送的正文
/// @returns SSE 响应或样本缺失错误
async fn respond(State(state): State<Arc<ModelState>>, Json(request): Json<Value>) -> Response {
    state.requests.lock().unwrap().push(request);
    match state.replies.lock().unwrap().pop_front() {
        Some((body, true)) => {
            let stream =
                futures_util::stream::once(std::future::ready(Ok::<_, std::io::Error>(body)))
                    .chain(futures_util::stream::pending());
            (
                [("content-type", "text/event-stream")],
                Body::from_stream(stream),
            )
                .into_response()
        }
        Some((body, false)) => ([("content-type", "text/event-stream")], body).into_response(),
        None => (StatusCode::BAD_REQUEST, "no model fixture for request").into_response(),
    }
}

/// 【插件测试】【服务描述】创建明确声明并授权模型、工具能力的外部包。
/// @param id 插件标识；source 为 Lua 源码；capabilities 为能力声明
/// @returns 可供真实注册入口加载的描述
pub(super) fn service_descriptor(id: &str, source: &str, capabilities: Value) -> PluginDescriptor {
    let mut plugin = descriptor(id, source);
    plugin.package.manifest.capabilities = serde_json::from_value(capabilities).unwrap();
    plugin.setting.grants = Some(plugin.package.manifest.capabilities.clone());
    plugin
}

/// 【插件测试】【服务注册】将测试插件安装到实际工具注册表。
/// @param registry 注册表；id 为插件标识；source 为源码；capabilities 为能力声明和授权
/// @returns 无，加载失败终止测试
pub(super) fn register_service(
    registry: &mut ToolRegistry,
    id: &str,
    source: &str,
    capabilities: Value,
) {
    register_descriptor(
        registry,
        service_descriptor(id, source, capabilities),
        Arc::new(FixtureHost::default()),
        false,
    )
    .unwrap();
}

/// 【插件测试】【模型工具建议】创建一项供应商函数调用增量。
/// @param index 调用顺序；name 为工具名称；arguments 为参数
/// @returns 可嵌入 SSE delta 的工具调用
pub(super) fn tool_call(index: usize, name: &str, arguments: Value) -> Value {
    json!({"index":index, "id":format!("call-{index}"), "type":"function",
        "function":{"name":name, "arguments":arguments.to_string()}})
}
