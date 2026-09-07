use super::Agent;
use crate::plugins::PluginDiagnostic;
use crate::tools::ToolRegistry;
use anyhow::Result;
use sai_plugin_runtime::PluginCommand;

impl Agent {
    /// 【插件】【会话目录】返回当前会话真正加载的插件和版本。
    /// @returns 当前插件列表，不执行新的 Lua 初始化
    pub(crate) fn active_plugins(&self) -> Vec<(String, String)> {
        self.tools.active_plugins()
    }

    /// 【插件】【会话命令】返回当前实例的直接用户命令。
    /// @returns 插件 ID 与命令定义
    pub(crate) fn plugin_commands(&self) -> Vec<(String, PluginCommand)> {
        self.tools.plugin_commands()
    }

    /// 【插件】【加载错误】提供由交互面展示的插件诊断。
    /// @returns 当前工具表的只读诊断
    pub(crate) fn plugin_diagnostics(&self) -> &[PluginDiagnostic] {
        self.tools.plugin_diagnostics()
    }

    /// 【插件】【命令执行表】命令临时进入执行表并复用当前 VM，不向模型公开命令工具。
    /// @param id 插件 ID；name 为包内命令名称
    /// @returns 共享会话实例和权限的临时执行表，以及内部命令工具名
    pub(crate) fn plugin_command_registry(
        &self,
        id: &str,
        name: &str,
    ) -> Result<(ToolRegistry, String)> {
        let spec = self.tools.plugin_command(id, name)?;
        let name = spec.name.clone();
        let mut registry = self.tools.clone();
        registry.register(spec);
        Ok((registry, name))
    }

    /// 【插件】【本地重新加载】替换本地注册表并保留已经发现的 MCP 工具。
    /// @param tools 使用当前配置、会话和权限重新建立的本地工具表
    /// @returns 工具复制与替换结果；旧插件注册随旧表释放
    pub(crate) fn replace_local_tools(&mut self, mut tools: ToolRegistry) -> Result<()> {
        for tool in self.tools.tool_infos() {
            if tool.name.starts_with("mcp_") && !tools.contains(&tool.name) {
                tools.register_from(&self.tools, &tool.name)?;
            }
        }
        self.replace_tools(tools);
        Ok(())
    }
}
