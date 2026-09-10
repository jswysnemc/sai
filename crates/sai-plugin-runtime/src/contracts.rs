use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 【插件】【工具权限】工具声明只能保持或收窄宿主权限。
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolAccess {
    #[default]
    ReadOnly,
    Writes,
    /// 【插件】【可选写入】普通调用需要写入权限，只读调用仍可执行受宿主限制的查询分支
    OptionalWrites,
}

/// 【插件】【工具契约】可序列化的模型工具定义，不包含 Lua 函数句柄。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PluginTool {
    pub name: String,
    pub description: String,
    pub parameters: Value,
    #[serde(default)]
    pub access: ToolAccess,
}

/// 【插件】【命令契约】直接由用户调用的插件命令。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PluginCommand {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub access: ToolAccess,
}

/// 【插件】【生命周期】稳定的宿主事件名称。
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    AgentStart,
    AgentEnd,
    TurnStart,
    TurnEnd,
    MessageStart,
    MessageEnd,
    ToolCall,
    ToolResult,
    ReplyEnd,
}

/// 【插件】【事件上下文】只传入本次事件允许插件读取的资料。
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct EventContext {
    pub session_id: String,
    pub workdir: String,
    #[serde(default)]
    pub storage_session_id: String,
    #[serde(default)]
    pub operation_id: String,
    #[serde(default)]
    pub data: Value,
}
