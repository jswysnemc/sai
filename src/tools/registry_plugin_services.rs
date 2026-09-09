use super::{ToolPermission, ToolRegistry};
use crate::config::AppConfig;
use crate::llm::OpenAiCompatibleClient;
use crate::paths::SaiPaths;
use crate::plugins::{PluginModelSource, PluginServices};
use anyhow::Result;
use sai_plugin_runtime::InvocationContext;
use std::sync::Arc;

impl ToolRegistry {
    /// 【插件】【实例身份】取得工具表实际持有的 VM 标识，用于检测旧注册表中的活动观察者。
    /// @param id 插件标识
    /// @returns 进程内实例标识；未加载时返回错误
    pub(crate) fn plugin_instance_id(&self, id: &str) -> Result<u64> {
        self.plugins.instance_id(id)
    }

    /// 【插件】【模型配置】为直接工具命令绑定配置，实际调用前不初始化模型。
    /// @param config 当前应用配置；paths 为凭据与应用目录
    /// @returns 无
    pub(crate) fn configure_plugin_model(&mut self, config: &AppConfig, paths: &SaiPaths) {
        self.plugin_model = Some(PluginModelSource::Config(Arc::new((
            config.clone(),
            paths.clone(),
        ))));
    }

    /// 【插件】【当前模型】Agent 或子任务选定客户端后覆盖配置来源，热切换立即生效。
    /// @param client 当前真实模型客户端
    /// @returns 无
    pub(crate) fn set_plugin_model_client(&mut self, client: &OpenAiCompatibleClient) {
        self.plugin_model = Some(PluginModelSource::Client(Arc::new(client.clone())));
    }

    /// 【插件】【工具归属】读取原注册表中的插件归属，不接受模型伪造来源。
    /// @param name 精确本地工具名称
    /// @returns 所属插件标识；原生工具或不存在的名称没有插件归属
    pub(crate) fn plugin_owner(&self, name: &str) -> Option<&str> {
        self.tools.get(name).and_then(|tool| tool.plugin_id())
    }

    /// 【插件】【调用服务】以当前工具白名单、权限及插件授权交集创建独立服务。
    /// @param id 工具所属插件；context 为可信调用上下文
    /// @returns 需要模型或工具能力时返回服务，普通 HTTP 插件沿用原有路径
    pub(super) fn plugin_services(
        &self,
        id: &str,
        context: &InvocationContext,
    ) -> Result<Option<Arc<PluginServices>>> {
        let grants = self.plugins.capabilities(id)?;
        if !grants.model && grants.tools.is_empty() {
            return Ok(None);
        }
        let names = grants
            .tools
            .iter()
            .filter(|name| {
                self.permission(name).is_ok_and(|permission| {
                    context.allow_writes || permission == ToolPermission::ReadOnly
                })
            })
            .cloned()
            .collect();
        // 【插件】【能力组合】保留 Agent 目录供被调用插件检查自己的授权，调用方仍只能使用 names
        let tools = self.clone();
        let model = grants.model.then(|| self.plugin_model.clone()).flatten();
        Ok(Some(Arc::new(PluginServices::new(
            id, tools, model, names, context,
        )?)))
    }
}
