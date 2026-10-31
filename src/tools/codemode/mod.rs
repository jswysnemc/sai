pub(crate) mod dispatch;
pub(crate) mod output;
pub(crate) mod process;

use crate::config::{CodeMode, CodeModeConfig};
use crate::tools::{ToolRegistry, ToolSpec};
use sai_codemode::catalog::CatalogEntry;

pub(crate) const NAME: &str = "codemode";
const DESCRIPTION: &str = "Execute JavaScript as an async function body with top-level await/return. Call tools.<name>(args), await Promise.all, filter results, and emit only useful output via text(value), return value or image(base64DataUrl). Tool calls retain session permissions and side effects; failures do not undo completed tools. No filesystem, network, Node APIs or timers exist in the JS runtime. Use ALL_TOOLS, await searchTools(query), or await describeTool(name) for discovery. store(key, value)/load(key) retain small branch-local JSON; store(key, undefined) deletes. Writes commit only on success. console.log/info/error/debug emit text. exit() completes successfully. Optional FIRST line: // @options: {\"max_output_tokens\":2000,\"timeout_ms\":60000}. Await every tool call; unawaited calls are discarded at completion. Tools return parsed JSON when possible, otherwise text; image attachments are available as images[].";

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

/// 【CodeMode】【档案边界】使用 config 注册脚本入口，返回无；独占白名单必须显式允许入口。
pub(crate) fn register_for_agent(registry: &mut ToolRegistry, config: &crate::config::AppConfig) {
    if config
        .agent_runtime
        .as_ref()
        .is_some_and(|runtime| runtime.exclusive)
        && !crate::config::whitelist_allows_tool(config, NAME)
    {
        registry.remove(NAME);
        return;
    }
    register(registry, &config.codemode);
}

/// 【CodeMode】【目录适配】从当前 registry 生成可调用工具目录，返回排除递归入口的目录。
pub(crate) fn catalog(registry: &ToolRegistry) -> Vec<CatalogEntry> {
    registry
        .definitions()
        .into_iter()
        .filter(|definition| !dispatch::is_gateway(&definition.function.name))
        .map(|definition| {
            let function = definition.function;
            CatalogEntry::new(function.name, function.description, function.parameters)
        })
        .collect()
}

/// 【CodeMode】【动态声明】使用当前 registry 与 config 生成模型声明，返回声明或空值。
pub(crate) fn definition(
    registry: &ToolRegistry,
    config: &CodeModeConfig,
) -> Option<crate::llm::ToolDefinition> {
    let mut definition = registry.definition(NAME)?;
    let entries = catalog(registry);
    definition.function.description = format!(
        "{DESCRIPTION}\n\nAvailable tool declarations (partial; use discovery for the rest):\n{}",
        sai_codemode::catalog::inline(&entries, config.inline_budget)
    );
    Some(definition)
}
