use crate::ToolAccess;
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 【插件】【模型消息】文本模型调用支持的消息角色。
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelRole {
    System,
    User,
    Assistant,
}

/// 【插件】【模型消息】插件自己维护的调查上下文，不包含宿主对话或供应商配置。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelMessage {
    pub role: ModelRole,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
}

/// 【插件】【模型请求】单次模型请求；工具只引用宿主已经授权的名称。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelRequest {
    pub messages: Vec<ModelMessage>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub stream_reasoning: bool,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

/// 【插件】【模型工具建议】模型返回的建议不会由宿主自动执行。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// 【插件】【模型用量】单次请求的供应商用量，不包含凭据。
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    #[serde(default)]
    pub cache_read_tokens: u64,
    #[serde(default)]
    pub cache_write_tokens: u64,
}

/// 【插件】【模型返回】文本、思考、工具建议与一次请求的用量。
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelResponse {
    pub content: String,
    pub reasoning: Option<String>,
    pub tool_calls: Vec<ModelToolCall>,
    pub usage: Option<ModelUsage>,
}

/// 【插件】【可用工具】调用所属任务实际暴露的工具契约。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HostTool {
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    pub description: String,
    pub parameters: Value,
    pub access: ToolAccess,
}

/// 【插件】【调用服务】宿主为一次回调绑定的模型与工具能力，生命周期短于插件实例。
#[async_trait]
pub trait InvocationServices: Send + Sync {
    /// 【插件】【工具目录】枚举已按父任务权限和插件声明收窄的工具。
    /// @returns 工具定义，不执行工具或模型请求
    fn tools(&self) -> Result<Vec<HostTool>>;

    /// 【插件】【工具执行】通过宿主原有授权、事件与审计路径执行工具。
    /// @param name 精确工具名；arguments 为 JSON 参数文本
    /// @returns 工具文本结果，权限拒绝或执行失败时返回错误
    async fn call_tool(&self, name: &str, arguments: &str) -> Result<String>;

    /// 【插件】【模型执行】调用当前任务的模型，供应商和凭据由宿主持有。
    /// @param request 插件消息及工具名称；max_bytes 为本次输出限制
    /// @returns 单次结果；不执行模型提出的工具调用
    async fn complete(&self, request: ModelRequest, max_bytes: usize) -> Result<ModelResponse>;
}
