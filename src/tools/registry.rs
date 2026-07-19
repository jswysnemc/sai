use crate::llm::{FunctionDefinition, ToolDefinition};
use crate::permission::PermissionProfile;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;

pub type ToolFuture = Pin<Box<dyn Future<Output = Result<String>> + Send>>;
pub type ToolHandler = Arc<dyn Fn(Value, ToolProgress) -> ToolFuture + Send + Sync>;

#[derive(Clone, Default)]
pub struct ToolProgress {
    sender: Option<mpsc::UnboundedSender<String>>,
}

impl ToolProgress {
    pub fn new(sender: mpsc::UnboundedSender<String>) -> Self {
        Self {
            sender: Some(sender),
        }
    }

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
    handler: ToolHandler,
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
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
            permission: ToolPermission::ReadOnly,
            handler: Arc::new(move |args, _progress| Box::pin(handler(args))),
        }
    }

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
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
            permission: ToolPermission::ReadOnly,
            handler: Arc::new(move |args, progress| Box::pin(handler(args, progress))),
        }
    }

    pub fn writes(mut self) -> Self {
        self.permission = ToolPermission::Writes;
        self
    }

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

    async fn call(&self, args: Value, progress: ToolProgress) -> Result<String> {
        (self.handler)(args, progress).await
    }
}

#[derive(Default, Clone)]
pub struct ToolRegistry {
    tools: HashMap<String, ToolSpec>,
    permission_profile: Option<PermissionProfile>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, tool: ToolSpec) {
        self.tools.insert(tool.name.clone(), tool);
    }

    /// 绑定当前会话使用的权限配置。
    ///
    /// 参数:
    /// - `profile`: 权限模式、工作区和审计日志
    ///
    /// 返回:
    /// - 无
    pub(crate) fn set_permission_profile(&mut self, profile: PermissionProfile) {
        self.permission_profile = Some(profile);
    }

    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(ToolSpec::definition).collect()
    }

    pub fn definitions_for_names(&self, names: &BTreeSet<String>) -> Vec<ToolDefinition> {
        names
            .iter()
            .filter_map(|name| self.tools.get(name))
            .map(ToolSpec::definition)
            .collect()
    }

    pub fn definitions_except(&self, excluded: &[&str]) -> Vec<ToolDefinition> {
        self.tools
            .values()
            .filter(|tool| !excluded.iter().any(|name| *name == tool.name))
            .map(ToolSpec::definition)
            .collect()
    }

    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    pub fn tool_infos(&self) -> Vec<ToolInfo> {
        let mut infos = self
            .tools
            .values()
            .map(|tool| ToolInfo {
                name: tool.name.clone(),
                description: tool.description.clone(),
                permission: tool.permission,
            })
            .collect::<Vec<_>>();
        infos.sort_by(|left, right| left.name.cmp(&right.name));
        infos
    }

    /// 克隆指定名称集合中的工具。
    ///
    /// 参数:
    /// - `allowed`: 允许复制到新注册表的工具名称
    ///
    /// 返回:
    /// - 仅包含允许工具的新注册表
    pub fn clone_filtered(&self, allowed: &[&str]) -> ToolRegistry {
        let mut registry = ToolRegistry::new();
        for name in allowed {
            if let Some(tool) = self.tools.get(*name) {
                registry.register(tool.clone());
            }
        }
        registry
    }

    /// 从另一个注册表复制指定工具。
    ///
    /// 参数:
    /// - `source`: 来源工具注册表
    /// - `name`: 工具名称
    ///
    /// 返回:
    /// - 工具不存在时返回错误
    pub(crate) fn register_from(&mut self, source: &ToolRegistry, name: &str) -> Result<()> {
        let tool = source
            .tools
            .get(name)
            .with_context(|| format!("unknown tool: {name}"))?;
        self.register(tool.clone());
        Ok(())
    }

    pub fn permission(&self, name: &str) -> Result<ToolPermission> {
        let name = local_tool_name(name);
        let Some(tool) = self.tools.get(name) else {
            bail!("unknown tool: {name}");
        };
        Ok(tool.permission)
    }

    /// 判断工具执行前是否需要交互式权限审计。
    ///
    /// 参数:
    /// - `name`: 工具名称
    /// - `arguments`: 原始 JSON 参数
    ///
    /// 返回:
    /// - 当前权限配置要求等待用户决定时返回 `true`
    pub(crate) fn requires_permission(&self, name: &str, arguments: &str) -> Result<bool> {
        let name = local_tool_name(name);
        let Some(tool) = self.tools.get(name) else {
            bail!("unknown tool: {name}");
        };
        let arguments = parse_arguments(arguments)?;
        Ok(self.permission_profile.as_ref().is_some_and(|profile| {
            profile.requires_interactive_audit(name, tool.permission, &arguments)
        }))
    }

    /// 记录工具权限请求已经展示给用户。
    pub(crate) fn record_permission_requested(&self, name: &str, arguments: &str) -> Result<()> {
        let arguments = parse_arguments(arguments)?;
        if let Some(profile) = &self.permission_profile {
            profile.record_requested(local_tool_name(name), &arguments);
        }
        Ok(())
    }

    /// 记录用户已经批准工具权限。
    ///
    /// 参数:
    /// - `name`: 工具名称
    /// - `arguments`: 原始工具参数
    ///
    /// 返回:
    /// - 参数解析和审计写入结果
    pub(crate) fn record_permission_approved(&self, name: &str, arguments: &str) -> Result<()> {
        let arguments = parse_arguments(arguments)?;
        if let Some(profile) = &self.permission_profile {
            profile.record_approved(local_tool_name(name), &arguments);
        }
        Ok(())
    }

    /// 记录用户拒绝工具权限及可选回复。
    pub(crate) fn record_permission_denied(
        &self,
        name: &str,
        arguments: &str,
        reply: Option<&str>,
    ) -> Result<()> {
        let arguments = parse_arguments(arguments)?;
        if let Some(profile) = &self.permission_profile {
            profile.record_denied(local_tool_name(name), &arguments, reply);
        }
        Ok(())
    }

    pub async fn call(&self, name: &str, arguments: &str) -> Result<String> {
        let name = local_tool_name(name);
        let Some(tool) = self.tools.get(name) else {
            bail!("unknown tool: {name}");
        };
        let mut args = parse_arguments(arguments)?;
        self.call_authorized(tool, name, &mut args, ToolProgress::default())
            .await
    }

    pub async fn call_with_progress(
        &self,
        name: &str,
        arguments: &str,
        sender: mpsc::UnboundedSender<String>,
    ) -> Result<String> {
        let name = local_tool_name(name);
        let Some(tool) = self.tools.get(name) else {
            bail!("unknown tool: {name}");
        };
        let mut args = parse_arguments(arguments)?;
        self.call_authorized(tool, name, &mut args, ToolProgress::new(sender))
            .await
    }

    /// 统一完成权限判定、沙盒标记注入和审计结果记录。
    ///
    /// 参数:
    /// - `tool`: 待执行工具定义
    /// - `name`: 本地工具名称
    /// - `args`: 已解析工具参数
    /// - `progress`: 工具进度通道
    ///
    /// 返回:
    /// - 工具执行结果
    async fn call_authorized(
        &self,
        tool: &ToolSpec,
        name: &str,
        args: &mut Value,
        progress: ToolProgress,
    ) -> Result<String> {
        if let Some(profile) = &self.permission_profile {
            let sandboxed = profile.authorize(name, tool.permission, args)?;
            if sandboxed {
                args.as_object_mut()
                    .context("tool arguments must be a JSON object")?
                    .insert("_sai_sandbox".to_string(), Value::Bool(true));
            }
        }
        let result = tool.call(args.clone(), progress).await;
        if let Some(profile) = &self.permission_profile {
            profile.record_result(name, args, &result);
        }
        result
    }
}

/// 解析工具参数，空参数按空对象处理。
fn parse_arguments(arguments: &str) -> Result<Value> {
    if arguments.trim().is_empty() {
        Ok(json!({}))
    } else {
        Ok(serde_json::from_str(arguments)?)
    }
}

/// 将协议层工具别名还原为本地注册名称。
fn local_tool_name(name: &str) -> &str {
    if name == "sai_web_search" {
        "web_search"
    } else {
        name
    }
}

pub fn empty_parameters() -> Value {
    json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permission::{PermissionProfile, PermissionProfileMode};
    use std::path::PathBuf;
    use std::sync::Mutex;

    /// 验证批准后的网络命令不会再注入沙箱标记。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[tokio::test]
    async fn approved_network_command_reaches_handler_without_sandbox_marker() {
        let received = Arc::new(Mutex::new(None));
        let handler_received = Arc::clone(&received);
        let mut registry = ToolRegistry::new();
        registry.register(
            ToolSpec::new(
                "run_command",
                "test",
                empty_parameters(),
                move |arguments| {
                    let handler_received = Arc::clone(&handler_received);
                    async move {
                        *handler_received.lock().unwrap() = Some(arguments);
                        Ok("ok".to_string())
                    }
                },
            )
            .writes(),
        );
        registry.set_permission_profile(PermissionProfile::new(
            PermissionProfileMode::Audited,
            PathBuf::from("/workspace/project"),
            None,
        ));
        let arguments = r#"{"command":"curl https://example.com"}"#;

        registry
            .record_permission_approved("run_command", arguments)
            .unwrap();
        registry.call("run_command", arguments).await.unwrap();

        let received = received.lock().unwrap();
        assert!(received
            .as_ref()
            .is_some_and(|arguments| arguments.get("_sai_sandbox").is_none()));
    }
}
