use super::{
    dsh_bash_shell, local_tool_name, parse_arguments, ToolOutput, ToolPermission, ToolProgress,
    ToolRegistry, ToolSpec, DSH_BASH_EXECUTION_ALIAS,
};
use crate::plugins::PluginServices;
use anyhow::{bail, Context, Result};
use serde_json::Value;
use tokio::sync::mpsc;

impl ToolRegistry {
    /// 【工具】【文本执行】解析参数并经过统一授权入口调用工具。
    /// @param name 为工具名或兼容别名；arguments 为 JSON 对象文本
    /// @returns 工具文本结果，不交付模型附件
    pub async fn call(&self, name: &str, arguments: &str) -> Result<String> {
        let requested_name = name;
        let name = local_tool_name(name);
        let Some(tool) = self.tools.get(name) else {
            bail!("unknown tool: {name}");
        };
        let mut args = parse_arguments(arguments)?;
        Ok(self
            .call_authorized(
                tool,
                name,
                &mut args,
                ToolProgress::default(),
                false,
                requested_name == DSH_BASH_EXECUTION_ALIAS,
            )
            .await?
            .content)
    }

    /// 【工具】【进度执行】在调用期间交付进度，并保留结果中的模型附件。
    /// @param name 为工具名称；arguments 为 JSON 对象文本；sender 为进度发送端
    /// @returns 工具文本及可选模型附件
    pub async fn call_with_progress(
        &self,
        name: &str,
        arguments: &str,
        sender: mpsc::UnboundedSender<String>,
    ) -> Result<ToolOutput> {
        let requested_name = name;
        let name = local_tool_name(name);
        let Some(tool) = self.tools.get(name) else {
            bail!("unknown tool: {name}");
        };
        let mut args = parse_arguments(arguments)?;
        self.call_authorized(
            tool,
            name,
            &mut args,
            ToolProgress::new(sender),
            true,
            requested_name == DSH_BASH_EXECUTION_ALIAS,
        )
        .await
    }

    /// 统一完成权限判定、沙盒标记注入和审计结果记录。
    ///
    /// 参数:
    /// - `tool`: 待执行工具定义
    /// - `name`: 本地工具名称
    /// - `args`: 已解析工具参数
    /// - `progress`: 工具进度通道
    /// - `accept_model_attachments`: 调用方是否会把临时附件提交给模型
    ///
    /// 返回:
    /// - 工具执行结果
    async fn call_authorized(
        &self,
        tool: &ToolSpec,
        name: &str,
        args: &mut Value,
        progress: ToolProgress,
        accept_model_attachments: bool,
        use_dsh_bash: bool,
    ) -> Result<ToolOutput> {
        let original_args = args.clone();
        if let Some(profile) = &self.permission_profile {
            // 网格工具按目标地址判定归属：投给别人的会话默认直接拒绝
            let scope = crate::tools::mesh::session_scope_for_call(
                name,
                args,
                &self.session_key,
                &self.session_id,
            );
            let sandboxed = profile.authorize_scoped(name, tool.permission, args, scope)?;
            if sandboxed {
                args.as_object_mut()
                    .context("tool arguments must be a JSON object")?
                    .insert("_sai_sandbox".to_string(), Value::Bool(true));
            }
        }
        if accept_model_attachments && name == "read_file" {
            args.as_object_mut()
                .context("tool arguments must be a JSON object")?
                .insert("_sai_model_attachments".to_string(), Value::Bool(true));
        }
        if use_dsh_bash {
            args.as_object_mut()
                .context("tool arguments must be a JSON object")?
                .insert(
                    "_sai_command_shell".to_string(),
                    Value::String(dsh_bash_shell()),
                );
        }
        // 【插件】【嵌套边界】1. 递归检查先于事件，旧注册表的活动观察者使用独立 VM
        if let Some(id) = tool.plugin_id() {
            PluginServices::check_invocation(id)?;
        }
        let plugins = self
            .plugins
            .fork_active(&PluginServices::active_instances())?;
        let mut context =
            self.plugin_context(progress.clone(), tool.permission == ToolPermission::Writes);
        let result = async {
            plugins.check_tool(name, &original_args, &context).await?;
            // 【插件】【参数隔离】宿主内部标记只交给原生工具，不能污染插件公开 Schema
            let call_args = if tool.plugin_id().is_some() {
                original_args.clone()
            } else {
                args.clone()
            };
            let services = match tool.plugin_id() {
                Some(id) => self.plugin_services(id, &context)?,
                None => None,
            };
            if let Some(services) = &services {
                context.services = Some(services.clone());
            }
            let operation = tool.call(call_args, progress, &plugins, context.clone());
            if let Some(services) = services {
                services
                    .events()
                    .agent_run(
                        serde_json::json!({"kind":"plugin", "plugin_id":tool.plugin_id()}),
                        operation,
                    )
                    .await
            } else {
                operation.await
            }
        }
        .await;
        if plugins.listens(sai_plugin_runtime::EventKind::ToolResult) {
            let output: String = match &result {
                Ok(output) => output.content.chars().take(16_384).collect(),
                Err(error) => format!("{error:#}").chars().take(16_384).collect(),
            };
            plugins
                .notify(
                    sai_plugin_runtime::EventKind::ToolResult,
                    &context,
                    serde_json::json!({
                        "name": name, "ok": result.is_ok(), "output": output,
                    }),
                )
                .await;
        }
        if let Some(profile) = &self.permission_profile {
            profile.record_result(
                name,
                args,
                result.as_ref().map(|output| output.content.as_str()),
            );
        }
        result
    }
}
