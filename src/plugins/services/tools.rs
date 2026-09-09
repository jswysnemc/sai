use super::{PluginServices, CALL_CHAIN};
use crate::tools::ToolPermission;
use anyhow::{bail, Result};
use sai_plugin_runtime::host::HostTool;
use sai_plugin_runtime::ToolAccess;

impl PluginServices {
    /// 【插件】【工具目录】保留父注册顺序，排除当前插件及全部祖先插件。
    /// @returns 实际可调用的定义
    pub(super) fn tool_catalog(&self) -> Result<Vec<HostTool>> {
        self.tools
            .definitions()
            .into_iter()
            .filter(|definition| self.allowed.contains(&definition.function.name))
            .filter(|definition| {
                self.tools
                    .plugin_owner(&definition.function.name)
                    .is_none_or(|owner| !self.chain.iter().any(|frame| frame.id == owner))
            })
            .map(|definition| {
                let access = match self.tools.permission(&definition.function.name)? {
                    ToolPermission::ReadOnly => ToolAccess::ReadOnly,
                    ToolPermission::Writes => ToolAccess::Writes,
                };
                let display_name =
                    crate::tools::readable_tool_name(&definition.function.name).to_string();
                Ok(HostTool {
                    name: definition.function.name,
                    display_name,
                    description: definition.function.description,
                    parameters: definition.function.parameters,
                    access,
                })
            })
            .collect()
    }

    /// 【插件】【工具执行】使用正常注册表调用，禁止借嵌套操作绕过权限或调用链检查。
    /// @param name 精确工具名；arguments 为原始 JSON 文本
    /// @returns 工具结果；失败不产生额外后台任务
    pub(super) async fn execute_tool(&self, name: &str, arguments: &str) -> Result<String> {
        if !self.tool_catalog()?.iter().any(|tool| tool.name == name) {
            bail!("plugin tool is not available: {name}");
        }
        CALL_CHAIN
            .scope(
                self.chain.clone(),
                crate::plugins::operation::scope(
                    &self.operation_id,
                    crate::runtime_cwd::scope(self.workdir.clone(), async {
                        self.tools.call(name, arguments).await
                    }),
                ),
            )
            .await
    }
}
