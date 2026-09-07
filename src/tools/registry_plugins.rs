use super::{ToolProgress, ToolRegistry, ToolSpec};
use crate::plugins::PluginSession;
use anyhow::{Context, Result};
use sai_plugin_runtime::InvocationContext;
use std::sync::Arc;

impl ToolRegistry {
    /// 【插件】【加载诊断】由实际注册入口保存错误，交给各交互面展示。
    /// @param diagnostics 本次发现和加载错误
    /// @returns 无
    pub(crate) fn set_plugin_diagnostics(
        &mut self,
        diagnostics: Vec<crate::plugins::PluginDiagnostic>,
    ) {
        self.plugin_diagnostics = diagnostics;
    }

    /// 【插件】【诊断读取】返回当前工具表的插件加载错误。
    /// @returns 独立错误列表，不在终端原始模式中直接写 stderr
    pub(crate) fn plugin_diagnostics(&self) -> &[crate::plugins::PluginDiagnostic] {
        &self.plugin_diagnostics
    }

    /// 【插件】【运行目录】读取当前会话已加载的插件及版本。
    /// @returns 按插件 ID 排序的列表
    pub(crate) fn active_plugins(&self) -> Vec<(String, String)> {
        self.plugins.active_plugins()
    }

    /// 【插件】【实例注册】把工具所属插件一并纳入注册表生命周期。
    /// @param source 来源插件集合；id 为待加入插件
    /// @returns 版本冲突检查结果
    pub(crate) fn add_plugin_session(&mut self, source: &PluginSession, id: &str) -> Result<()> {
        self.plugins.inherit(source, id)
    }

    /// 【插件】【Agent 隔离】创建新 Agent 时重建全部 Lua 状态。
    /// @param session_id 宿主状态存储提供的真实会话 ID
    /// @returns 新实例加载结果；同一 Agent 的注册表克隆无需调用此方法
    pub(crate) fn start_plugin_session(&mut self, session_id: &str) -> Result<()> {
        self.plugins = self.plugins.fork()?;
        self.plugin_session_id = Some(session_id.to_string());
        Ok(())
    }

    /// 【插件】【会话延续】替换同一 Agent 的工具表时复用未变更插件。
    /// @param previous 当前 Agent 原来的注册表
    /// @returns 无；禁用或更新的包不会保留旧实例
    pub(crate) fn continue_plugin_session(&mut self, previous: &Self) {
        self.plugins.preserve_from(&previous.plugins);
        self.plugin_session_id = previous.plugin_session_id.clone();
    }

    /// 【插件】【命令目录】列出当前启用插件的用户命令。
    /// @returns 插件 ID 和命令定义，不混入供应商工具列表
    pub(crate) fn plugin_commands(&self) -> Vec<(String, sai_plugin_runtime::PluginCommand)> {
        self.plugins.commands()
    }

    /// 【插件】【命令定位】为直接用户命令创建可复用现有权限流程的工具定义。
    /// @param id 插件 ID；name 为命令名称
    /// @returns 独立命令定义；未启用或未知命令返回错误
    pub(crate) fn plugin_command(&self, id: &str, name: &str) -> Result<ToolSpec> {
        let (_, definition) = self
            .plugins
            .commands()
            .into_iter()
            .find(|(plugin, command)| plugin == id && command.name == name)
            .with_context(|| format!("plugin command not found: {id}/{name}"))?;
        Ok(ToolSpec::plugin_command(id, &definition))
    }

    /// 【插件】【生命周期】固定当前插件实例，避免事件分发借用整个 Agent。
    /// @returns 使用当前会话及工作目录的事件分发器
    pub(crate) fn plugin_events(&self) -> crate::plugins::PluginEvents {
        crate::plugins::PluginEvents::new(
            self.plugins.clone(),
            self.plugin_context(ToolProgress::default(), false),
        )
    }

    /// 【插件】【可信上下文】从宿主会话和任务工作目录构造调用资料。
    /// @param progress 进度通道；allow_writes 表示已通过权限检查的写入工具
    /// @returns 模型参数无法覆盖的插件调用上下文
    pub(super) fn plugin_context(
        &self,
        progress: ToolProgress,
        allow_writes: bool,
    ) -> InvocationContext {
        InvocationContext {
            session_id: self
                .plugin_session_id
                .as_ref()
                .unwrap_or(&self.session_id)
                .clone(),
            workdir: crate::runtime_cwd::current_dir()
                .map(|path| path.display().to_string())
                .unwrap_or_default(),
            allow_writes,
            progress: Some(Arc::new(move |message| progress.report(message))),
        }
    }
}
