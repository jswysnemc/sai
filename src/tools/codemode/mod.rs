pub(crate) mod dispatch;
pub(crate) mod output;
pub(crate) mod process;

use crate::config::{CodeMode, CodeModeConfig};
use crate::tools::{ToolRegistry, ToolSpec};
use sai_codemode::catalog::CatalogEntry;

pub(crate) const NAME: &str = "codemode";
const DESCRIPTION: &str = "Execute JavaScript as an async function body with top-level await/return. Prefer native calls for simple operations; use scripts for loops, dependent calls and filtering intermediate results. Call tools[name](args) using the SAME names, descriptions and argument schemas already provided in native tool definitions or load/Jev results. Only exposed tools are callable; load deferred tools through the native loading/discovery tools before starting a script. ALL_TOOLS, searchTools(query) and describeTool(name) only inspect this exposed set; they do not discover or load hidden tools. Await calls, use Promise.all where appropriate, and emit useful output via text(value), return value or image(base64DataUrl). Tool calls retain session permissions; failures do not undo completed side effects. No filesystem, network, Node APIs or timers exist in the JS runtime. store(key, value)/load(key) retain small branch-local JSON; this load reads stored values, not tool schemas. store(key, undefined) deletes. Store changes commit only on success. console.log/info/error/debug emit text. exit() completes successfully. Optional FIRST line: // @options: {\"max_output_tokens\":2000,\"timeout_ms\":60000}. Unawaited calls are discarded at completion. Tools return parsed JSON when possible, otherwise text; image attachments are available as images[].";

/// 【CodeMode】【工具注册】向 registry 注册入口声明，config 决定是否启用；返回无。
pub(crate) fn register(registry: &mut ToolRegistry, config: &CodeModeConfig) {
    if config.mode == CodeMode::Off {
        registry.remove(NAME);
        return;
    }
    registry.register(ToolSpec::new(
        NAME,
        DESCRIPTION,
        serde_json::json!({
            "type": "object", "properties": {"code": {"type": "string", "maxLength": 1048576}},
            "required": ["code"], "additionalProperties": false
        }),
        |_| async { anyhow::bail!("codemode requires the active Agent execution bridge") },
    ));
}

/// 【CodeMode】【会话入口】按会话模式注册脚本入口，嵌套目录沿用已过滤的注册表
/// 参数: registry 为会话工具，config 为脚本配置，mode 为会话模式；返回: 无
pub(crate) fn register_for_agent(
    registry: &mut ToolRegistry,
    config: &crate::config::AppConfig,
    mode: crate::agent::AgentMode,
) {
    let mut execution = config.codemode.clone();
    execution.mode = mode.codemode();
    register(registry, &execution);
}

/// 【CodeMode】【目录适配】参数 definitions 为当前暴露的声明；返回排除递归和发现入口的目录。
pub(crate) fn catalog(
    definitions: impl IntoIterator<Item = crate::llm::ToolDefinition>,
) -> Vec<CatalogEntry> {
    definitions
        .into_iter()
        .filter(|definition| !dispatch::is_gateway(&definition.function.name))
        .map(|definition| {
            let function = definition.function;
            CatalogEntry::new(function.name, function.description, function.parameters)
        })
        .collect()
}

/// 【CodeMode】【入口声明】参数 registry 为注册表，config 保留配置接口；返回不含业务说明的脚本声明。
pub(crate) fn definition(
    registry: &ToolRegistry,
    _config: &CodeModeConfig,
) -> Option<crate::llm::ToolDefinition> {
    registry.definition(NAME)
}
