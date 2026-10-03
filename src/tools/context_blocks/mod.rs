mod definitions;
mod handlers;

#[cfg(test)]
mod tests;

use crate::config::AppConfig;
use crate::state::StateStore;
use crate::tools::{ToolRegistry, ToolSpec};

pub(crate) const NAMES: [&str; 4] = [
    "context_status",
    "compress_context",
    "search_context",
    "restore_context",
];

/// 【上下文】【工具识别】判断工具是否负责上下文管理
/// 参数: name 为真实工具名；返回是否需要保护其结果
pub(crate) fn is_context_tool(name: &str) -> bool {
    NAMES.contains(&name)
}

/// 【上下文】【会话绑定】注册当前会话的实验工具，关闭开关时移除旧绑定
/// 参数: registry 为注册表，state 为会话，config 为配置；返回无
pub(crate) fn register(registry: &mut ToolRegistry, state: &StateStore, config: &AppConfig) {
    for name in NAMES {
        registry.remove(name);
    }
    if !config.context.experimental_context_blocks
        || !config.tools.enabled
        || config.agent.engine.is_external()
        || !config.active_model_tools_enabled().unwrap_or(false)
    {
        return;
    }
    for definition in definitions::definitions() {
        let name = definition.name;
        if !crate::config::whitelist_allows_tool(config, name) {
            continue;
        }
        let state = state.clone();
        // 1. 【上下文】【工具注册】只变更会话请求视图，不授予文件或命令写入权限
        registry.register(ToolSpec::new(
            name,
            definition.description,
            definition.parameters,
            move |args| {
                let state = state.clone();
                async move { handlers::execute(&state, name, args) }
            },
        ));
    }
}

/// 【上下文】【配置目录】注册只含元数据的工具，不创建会话或访问数据库
/// 参数: registry 为配置目录注册表；返回无
pub(crate) fn register_catalog(registry: &mut ToolRegistry) {
    for definition in definitions::definitions() {
        registry.register(ToolSpec::new(
            definition.name,
            definition.description,
            definition.parameters,
            |_| async { anyhow::bail!("catalog tools cannot execute") },
        ));
    }
}
