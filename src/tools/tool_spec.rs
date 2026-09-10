use super::descriptions::tool_description;
use crate::llm::{FunctionDefinition, ToolDefinition};
use anyhow::Result;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;

pub type ToolFuture = Pin<Box<dyn Future<Output = Result<ToolOutput>> + Send>>;
pub type ToolHandler = Arc<dyn Fn(Value, ToolProgress) -> ToolFuture + Send + Sync>;

#[derive(Clone)]
enum ToolBackend {
    Native(ToolHandler),
    LuaTool { plugin_id: String, name: String },
    LuaCommand { plugin_id: String, name: String },
}

/// 工具希望在下一次模型请求中附加的图片。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolModelAttachment {
    pub(crate) image_url: String,
    pub(crate) source: String,
    pub(crate) prompt: String,
}

impl ToolModelAttachment {
    /// 创建模型图片附件。
    ///
    /// 参数:
    /// - `image_url`: 图片 data URL 或远程 URL
    /// - `source`: 图片来源路径或标识
    /// - `prompt`: 当前模型分析图片时使用的提示
    ///
    /// 返回:
    /// - 模型图片附件
    pub(crate) fn new(
        image_url: impl Into<String>,
        source: impl Into<String>,
        prompt: impl Into<String>,
    ) -> Self {
        Self {
            image_url: image_url.into(),
            source: source.into(),
            prompt: prompt.into(),
        }
    }
}

/// 工具文本结果和仅供下一次模型请求使用的附件。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolOutput {
    pub(crate) content: String,
    pub(crate) model_attachments: Vec<ToolModelAttachment>,
}

impl ToolOutput {
    /// 创建不包含模型附件的普通工具结果。
    ///
    /// 参数:
    /// - `content`: 工具文本结果
    ///
    /// 返回:
    /// - 普通工具结果
    pub(crate) fn text(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            model_attachments: Vec::new(),
        }
    }

    /// 为工具结果附加下一次模型请求使用的图片。
    ///
    /// 参数:
    /// - `attachments`: 图片附件列表
    ///
    /// 返回:
    /// - 包含模型图片附件的工具结果
    pub(crate) fn with_model_attachments(
        mut self,
        attachments: impl IntoIterator<Item = ToolModelAttachment>,
    ) -> Self {
        self.model_attachments.extend(attachments);
        self
    }
}

#[derive(Clone, Default)]
pub struct ToolProgress {
    sender: Option<mpsc::UnboundedSender<String>>,
}

impl ToolProgress {
    /// 【工具】【进度构造】绑定调用方提供的消息通道。
    /// @param sender 为文本进度发送端
    /// @returns 当前工具调用的进度句柄
    pub fn new(sender: mpsc::UnboundedSender<String>) -> Self {
        Self {
            sender: Some(sender),
        }
    }

    /// 【工具】【进度发送】向调用方交付中间状态，通道关闭时忽略消息。
    /// @param message 为进度文本
    /// @returns 无
    pub fn report(&self, message: impl Into<String>) {
        if let Some(sender) = &self.sender {
            let _ = sender.send(message.into());
        }
    }
}

#[derive(Clone)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,
    pub permission: ToolPermission,
    handler: ToolBackend,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolPermission {
    ReadOnly,
    Writes,
}

#[derive(Clone, Debug)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub permission: ToolPermission,
}

impl ToolSpec {
    /// 【工具】【原生定义】构造返回文本的只读原生工具。
    /// @param name、description 为名称与说明；parameters 为参数 Schema；handler 为业务回调
    /// @returns 默认只读的工具定义
    pub fn new<F, Fut>(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: Value,
        handler: F,
    ) -> Self
    where
        F: Fn(Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<String>> + Send + 'static,
    {
        let name = name.into();
        let fallback_description = description.into();
        let description = tool_description(&name, &fallback_description);
        Self {
            name,
            description,
            parameters,
            permission: ToolPermission::ReadOnly,
            handler: ToolBackend::Native(Arc::new(move |args, _progress| {
                let future = handler(args);
                Box::pin(async move { future.await.map(ToolOutput::text) })
            })),
        }
    }

    /// 创建可以返回下一次模型请求附件的工具。
    ///
    /// 参数:
    /// - `name`: 工具名称
    /// - `description`: 工具说明
    /// - `parameters`: JSON Schema 参数定义
    /// - `handler`: 返回结构化工具结果的异步处理函数
    ///
    /// 返回:
    /// - 工具定义
    pub(crate) fn new_with_output<F, Fut>(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: Value,
        handler: F,
    ) -> Self
    where
        F: Fn(Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<ToolOutput>> + Send + 'static,
    {
        let name = name.into();
        let fallback_description = description.into();
        let description = tool_description(&name, &fallback_description);
        Self {
            name,
            description,
            parameters,
            permission: ToolPermission::ReadOnly,
            handler: ToolBackend::Native(Arc::new(move |args, _progress| Box::pin(handler(args)))),
        }
    }

    /// 【工具】【进度定义】构造可以报告中间进度的原生工具。
    /// @param name、description 为名称与说明；parameters 为参数 Schema；handler 接收参数与进度句柄
    /// @returns 默认只读的工具定义
    pub fn new_with_progress<F, Fut>(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: Value,
        handler: F,
    ) -> Self
    where
        F: Fn(Value, ToolProgress) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<String>> + Send + 'static,
    {
        let name = name.into();
        let fallback_description = description.into();
        let description = tool_description(&name, &fallback_description);
        Self {
            name,
            description,
            parameters,
            permission: ToolPermission::ReadOnly,
            handler: ToolBackend::Native(Arc::new(move |args, progress| {
                let future = handler(args, progress);
                Box::pin(async move { future.await.map(ToolOutput::text) })
            })),
        }
    }

    /// 【工具】【写入声明】将当前定义标记为需要写入权限。
    /// @returns 更新权限后的工具定义，无额外参数
    pub fn writes(mut self) -> Self {
        self.permission = ToolPermission::Writes;
        self
    }

    /// 【工具】【模型契约】复制供模型请求使用的工具名称、说明和参数。
    /// @returns 不包含执行后端的协议定义，无额外参数
    pub fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            kind: "function",
            function: FunctionDefinition {
                name: self.name.clone(),
                description: self.description.clone(),
                parameters: self.parameters.clone(),
            },
        }
    }

    /// 【插件】【工具适配】保存注册元数据和定位信息，实例由当前会话在执行时提供。
    /// @param id 插件标识；name 为公开名称；definition 为 Lua 提交的契约
    /// @returns 使用原始插件说明的工具定义，不捕获 Lua VM
    pub(crate) fn plugin(
        id: &str,
        name: String,
        definition: &sai_plugin_runtime::PluginTool,
    ) -> Self {
        Self {
            name,
            description: definition.description.clone(),
            parameters: definition.parameters.clone(),
            permission: plugin_permission(definition.access),
            handler: ToolBackend::LuaTool {
                plugin_id: id.to_string(),
                name: definition.name.clone(),
            },
        }
    }

    /// 【插件】【命令适配】将用户命令交给同一权限和审计入口，默认不加入模型工具目录。
    /// @param id 插件标识；definition 为命令定义
    /// @returns 仅供直接命令执行使用的工具契约
    pub(crate) fn plugin_command(id: &str, definition: &sai_plugin_runtime::PluginCommand) -> Self {
        Self {
            name: format!("lua_command__{id}__{}", definition.name),
            description: definition.description.clone(),
            parameters: serde_json::json!({"type":"object","properties":{"arguments":{"type":"string"}},"required":["arguments"],"additionalProperties":false}),
            permission: plugin_permission(definition.access),
            handler: ToolBackend::LuaCommand {
                plugin_id: id.to_string(),
                name: definition.name.clone(),
            },
        }
    }

    /// 【插件】【归属查询】取得需要随工具复制的插件标识。
    /// @returns 原生工具返回 None，Lua 工具和命令返回插件 ID
    pub(super) fn plugin_id(&self) -> Option<&str> {
        match &self.handler {
            ToolBackend::Native(_) => None,
            ToolBackend::LuaTool { plugin_id, .. } | ToolBackend::LuaCommand { plugin_id, .. } => {
                Some(plugin_id)
            }
        }
    }

    /// 【工具】【执行分派】原生工具沿用进度接口，Lua 工具使用会话所有的实例。
    /// @param args 为已校验参数；progress 为进度；plugins 为当前会话实例；context 为宿主可信上下文
    /// @returns 工具文本及可选模型附件
    pub(super) async fn call(
        &self,
        args: Value,
        progress: ToolProgress,
        plugins: &crate::plugins::PluginSession,
        context: sai_plugin_runtime::InvocationContext,
    ) -> Result<ToolOutput> {
        let output = match &self.handler {
            ToolBackend::Native(handler) => return handler(args, progress).await,
            ToolBackend::LuaTool { plugin_id, name } => {
                plugins.call_tool(plugin_id, name, args, context).await?
            }
            ToolBackend::LuaCommand { plugin_id, name } => {
                let arguments = args
                    .get("arguments")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow::anyhow!("plugin command arguments must be text"))?;
                plugins
                    .call_command(plugin_id, name, arguments, context)
                    .await?
            }
        };
        Ok(ToolOutput::text(output))
    }
}

/// 【插件】【权限映射】把运行时权限声明映射到 Sai 的统一工具权限。
/// @param access 为 Lua 工具或命令声明的权限
/// @returns Sai 已有的只读或写入权限
fn plugin_permission(access: sai_plugin_runtime::ToolAccess) -> ToolPermission {
    match access {
        sai_plugin_runtime::ToolAccess::ReadOnly => ToolPermission::ReadOnly,
        sai_plugin_runtime::ToolAccess::Writes | sai_plugin_runtime::ToolAccess::OptionalWrites => {
            ToolPermission::Writes
        }
    }
}
