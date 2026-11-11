use super::ToolVisibility;
use crate::config::{CodeMode, CodeModeConfig};
use crate::llm::ToolDefinition;
use crate::tools::{self, ToolRegistry};
use sai_codemode::catalog::CatalogEntry;
use std::collections::BTreeSet;

impl ToolVisibility {
    /// 【CodeMode】【发现入口】参数 name 为工具名；返回是否属于独占模式仍保留的加载入口。
    pub(crate) fn is_discovery_gateway(name: &str) -> bool {
        matches!(name, tools::LOAD_NAME | tools::REQUEST_CAPABILITY_NAME)
    }

    /// 【CodeMode】【共享可见性】参数 name 为工具名；返回是否已按原生规则暴露给本会话。
    pub(crate) fn is_script_tool_exposed(&self, name: &str) -> bool {
        !tools::codemode::dispatch::is_gateway(name)
            && ((self.is_anchor_enabled() && super::deepseek_anchor::is_provider_tool(name))
                || self.is_tool_exposed(name))
    }

    /// 【CodeMode】【共享目录】参数 registry 为完整注册表；返回原生声明与已加载工具的去重目录。
    pub(crate) fn codemode_catalog(&self, registry: &ToolRegistry) -> Vec<CatalogEntry> {
        let mut seen = BTreeSet::new();
        let loaded = self.loaded_tool_names().into_iter().collect();
        tools::codemode::catalog(
            self.direct_definitions(registry)
                .into_iter()
                .chain(registry.definitions_for_names(&loaded))
                .filter(|definition| seen.insert(definition.function.name.clone())),
        )
    }

    /// 【CodeMode】【独占声明】参数为入口声明、注册表和配置；仅独占模式内联未在加载结果出现的常驻说明。
    pub(super) fn codemode_definition(
        &self,
        mut definition: ToolDefinition,
        registry: &ToolRegistry,
        config: &CodeModeConfig,
    ) -> ToolDefinition {
        if config.mode == CodeMode::Only {
            let resident = tools::codemode::catalog(self.direct_definitions(registry));
            definition.function.description.push_str(
                "\nBusiness tools must run through this script. Native load/request_capability remain available. Resident declarations (partial; describeTool can inspect the rest):\n",
            );
            definition
                .function
                .description
                .push_str(&sai_codemode::catalog::inline(
                    &resident,
                    config.inline_budget,
                ));
        }
        definition
    }

    /// 【工具加载】【调用提示】无参数；返回与当前工具方式一致的已加载工具调用约定。
    pub(crate) fn loaded_tool_instruction(&self) -> &'static str {
        match self.codemode.as_ref().map(|config| config.mode) {
            Some(CodeMode::Only) => "Call loaded tools inside codemode as tools[name](args), using the exact name and returned argument schema. Direct business tool calls and invoke_tool are disabled.",
            Some(CodeMode::On) => "Call loaded tools through invoke_tool or inside codemode as tools[name](args). Both use the exact same name and returned argument schema. Do not emit a native call to a deferred tool name.",
            _ => "Call loaded tools through invoke_tool with the exact name and returned argument schema; do not emit a native call to a deferred tool name.",
        }
    }
}
