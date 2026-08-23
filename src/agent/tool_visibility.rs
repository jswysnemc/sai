use super::deepseek_anchor;
use super::load_request::{LoadRequest, LoadType};
use crate::config::AppConfig;
use crate::llm::ToolDefinition;
use crate::paths::SaiPaths;
use crate::tools::{self, ToolRegistry};
use anyhow::{bail, Result};
use serde_json::json;
use std::collections::BTreeSet;

#[derive(Clone)]
pub(crate) struct ToolVisibility {
    /// 非空表示启用统一渐进网关；具体内容保留用于兼容现有 Agent 配置
    deferred: Vec<String>,
    loaded: BTreeSet<String>,
    /// 工具首次被 load 的顺序，用于稳定持久化状态和加载结果
    loaded_order: Vec<String>,
    /// 本会话已经全文 load 过的 skill 名称
    pub(super) loaded_skills: BTreeSet<String>,
    /// skill 首次被 load 的顺序
    pub(super) loaded_skill_order: Vec<String>,
    /// DeepSeek Anchored Standard 是否控制当前会话的工具目录。
    anchor_enabled: bool,
    /// false 表示请求 #1 尚未产生持久 assistant/tool 信号。
    anchor_promoted: bool,
}

impl ToolVisibility {
    /// 创建工具可见性状态。
    ///
    /// 参数:
    /// - `deferred`: 当前 Agent 的渐进配置，非空时启用统一网关
    ///
    /// 返回:
    /// - 新的工具可见性状态
    pub(crate) fn new(deferred: Vec<String>) -> Self {
        Self {
            deferred,
            loaded: BTreeSet::new(),
            loaded_order: Vec::new(),
            loaded_skills: BTreeSet::new(),
            loaded_skill_order: Vec::new(),
            anchor_enabled: false,
            anchor_promoted: false,
        }
    }

    /// 按运行期 Agent 覆盖创建工具可见性状态。
    ///
    /// 参数:
    /// - `config`: 当前应用配置
    ///
    /// 返回:
    /// - 依据 `agent_runtime.deferred_tools` 构造的可见性状态
    pub(crate) fn from_config(config: &AppConfig) -> Self {
        Self::new(config.agent_deferred_tools().to_vec())
    }

    /// 按配置创建可见性状态，并启用 DeepSeek Anchored Standard。
    pub(crate) fn from_config_with_anchor(
        config: &AppConfig,
        anchor_enabled: bool,
        anchor_promoted: bool,
    ) -> Self {
        let mut visibility = Self::from_config(config);
        if anchor_enabled {
            visibility.deferred = vec![tools::DEFERRED_ALL_EXCEPT_ANCHOR_BOOTSTRAP.to_string()];
            visibility.anchor_enabled = true;
            visibility.anchor_promoted = anchor_promoted;
        }
        visibility
    }

    /// 返回注册渐进发现网关时使用的延迟集合。
    pub(crate) fn deferred_tools(&self) -> &[String] {
        &self.deferred
    }

    /// 判断当前请求是否仍处于锚定首轮。
    pub(crate) fn is_anchor_bootstrap(&self) -> bool {
        self.anchor_enabled && !self.anchor_promoted
    }

    /// 判断当前会话是否使用 dsh Anchored Standard 工具适配层。
    pub(crate) fn is_anchor_enabled(&self) -> bool {
        self.anchor_enabled
    }

    /// 晋升到 resident 工具目录，返回阶段是否发生变化。
    pub(crate) fn promote_anchor(&mut self) -> bool {
        if !self.is_anchor_bootstrap() {
            return false;
        }
        self.anchor_promoted = true;
        true
    }

    /// 判断当前 Agent 是否启用了渐进式加载。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 延迟集合非空时返回 true
    pub(crate) fn is_progressive(&self) -> bool {
        !self.deferred.is_empty()
    }

    /// 计算当前应暴露给模型的工具定义。
    ///
    /// 参数:
    /// - `registry`: 完整工具注册表
    ///
    /// 返回:
    /// - 当前可见的工具定义列表
    pub(crate) fn definitions(&self, registry: &ToolRegistry) -> Vec<ToolDefinition> {
        if self.anchor_enabled {
            let mut definitions = deepseek_anchor::definitions();
            if self.anchor_promoted {
                let names = [tools::LOAD_NAME, tools::INVOKE_NAME]
                    .into_iter()
                    .map(str::to_string)
                    .collect::<BTreeSet<_>>();
                definitions.extend(registry.definitions_for_names(&names));
            }
            return definitions;
        }
        if !self.is_progressive() {
            return registry.definitions();
        }
        // 1. 基础工具与未配置为延迟的工具保留原生 Schema，延迟工具通过固定网关调用
        let names = tools::progressive::visible_tool_names(registry, &self.deferred);
        registry.definitions_for_names(&names)
    }

    /// 判断工具当前是否允许被模型调用。
    ///
    /// 参数:
    /// - `name`: 工具名称
    ///
    /// 返回:
    /// - 当前是否可见并允许调用
    pub(crate) fn is_visible(&self, name: &str) -> bool {
        if self.anchor_enabled
            && (deepseek_anchor::is_provider_tool(name) || deepseek_anchor::is_execution_tool(name))
        {
            return true;
        }
        if self.is_anchor_bootstrap() {
            return false;
        }
        !self.is_progressive()
            || name == tools::LOAD_NAME
            || name == tools::INVOKE_NAME
            || !self.requires_load(name)
            || self.loaded.contains(name)
    }

    /// 判断真实工具是否需要先通过 load 加载。
    ///
    /// 参数:
    /// - `name`: 工具名称
    ///
    /// 返回:
    /// - 当前配置下是否属于延迟工具
    pub(crate) fn requires_load(&self, name: &str) -> bool {
        self.is_progressive() && tools::progressive::is_deferred_tool(name, &self.deferred)
    }

    /// 判断当前工具调用是否为加载工具调用。
    ///
    /// 参数:
    /// - `name`: 工具名称
    ///
    /// 返回:
    /// - 是否为 `load`
    pub(crate) fn is_loader_call(&self, name: &str) -> bool {
        name == tools::LOAD_NAME
    }

    /// 判断当前工具调用是否为统一调用外壳。
    ///
    /// 参数:
    /// - `name`: 工具名称
    ///
    /// 返回:
    /// - 渐进模式下是否为 `invoke_tool`
    pub(crate) fn is_invoker_call(&self, name: &str) -> bool {
        self.is_progressive() && name == tools::INVOKE_NAME
    }

    /// 恢复已经加载过的工具集合。
    ///
    /// 参数:
    /// - `registry`: 当前完整工具注册表
    /// - `names`: 上一轮保存的已加载工具名称
    ///
    /// 返回:
    /// - 无
    pub(crate) fn restore_loaded_tools(&mut self, registry: &ToolRegistry, names: &[String]) {
        self.loaded.clear();
        self.loaded_order.clear();
        if !self.is_progressive() {
            return;
        }
        for name in names {
            if registry.contains(name) && self.is_loadable_tool(name) {
                if self.loaded.insert(name.clone()) {
                    self.loaded_order.push(name.clone());
                }
            }
        }
    }

    /// 判断工具是否属于当前 Agent 的延迟集合。
    ///
    /// 参数:
    /// - `name`: 工具名称
    ///
    /// 返回:
    /// - 是否需要 load 后才暴露
    fn is_loadable_tool(&self, name: &str) -> bool {
        self.requires_load(name)
    }

    /// 获取已经额外加载的工具名称。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 已加载工具名称列表
    pub(crate) fn loaded_tool_names(&self) -> Vec<String> {
        self.loaded_order.clone()
    }

    /// 判断工具是否已经通过渐进网关加载。
    pub(crate) fn is_loaded(&self, name: &str) -> bool {
        self.loaded.contains(name)
    }

    /// 按加载工具参数更新可见工具集合。
    ///
    /// 参数:
    /// - `registry`: 完整工具注册表
    /// - `arguments`: `load` 的 JSON 参数
    /// - `config`: 当前应用配置
    /// - `paths`: 应用目录路径集合
    ///
    /// 返回:
    /// - 给模型的加载结果说明
    pub(crate) fn load_from_arguments(
        &mut self,
        registry: &ToolRegistry,
        arguments: &str,
        config: &AppConfig,
        paths: &SaiPaths,
    ) -> Result<String> {
        let request = LoadRequest::parse(arguments)?;
        match request.resource_type {
            LoadType::Skill => self.load_skills(&request.keywords, config, paths),
            LoadType::Tool => self.load_requested_tools(registry, &request.keywords),
        }
    }

    /// 加载多个工具并返回固定的 `tools` 数组。
    ///
    /// 参数:
    /// - `registry`: 完整工具注册表
    /// - `keywords`: 要加载的工具名称
    ///
    /// 返回:
    /// - 包含工具名称和加载状态的 JSON
    fn load_requested_tools(
        &mut self,
        registry: &ToolRegistry,
        keywords: &[String],
    ) -> Result<String> {
        let result = self.load_tools(registry, keywords)?;
        let already_loaded = result.is_already_loaded_request();
        let instruction = if already_loaded {
            "The requested schemas are available in this result. Do not call load for these targets again; call invoke_tool with the exact tool name and matching arguments."
        } else {
            "The requested tool schemas are now available. Call invoke_tool with the exact tool name and arguments matching its returned schema; do not emit a direct call to the concrete tool."
        };
        let definitions = keywords
            .iter()
            .map(|name| {
                let status = if result.newly_loaded_tools.contains(name) {
                    "loaded"
                } else {
                    "already_loaded"
                };
                let definition = registry
                    .definition(name)
                    .expect("validated loaded tool must have a definition");
                json!({"name": name, "status": status, "definition": definition})
            })
            .collect::<Vec<_>>();
        Ok(serde_json::to_string_pretty(&json!({
            "ok": true,
            "tools": definitions,
            "already_loaded": already_loaded,
            "currently_loaded_tools": self.loaded_tool_names(),
            "instruction": instruction,
        }))?)
    }

    /// 原子加载多个工具。
    ///
    /// 参数:
    /// - `registry`: 完整工具注册表
    /// - `names`: 已经去重的工具名称列表
    ///
    /// 返回:
    /// - 本次请求新增和此前已经加载的工具名称
    fn load_tools(&mut self, registry: &ToolRegistry, names: &[String]) -> Result<ToolLoadResult> {
        // 1. 在更新状态前完整校验，避免批量请求出现部分加载
        for name in names {
            if !registry.contains(name) {
                bail!("unknown tool: {name}");
            }
            if name == tools::LOAD_NAME || name == tools::INVOKE_NAME {
                bail!("tool cannot be loaded through the progressive gateway: {name}");
            }
            if !self.is_loadable_tool(name) {
                bail!("tool {name} is already directly available; call it directly without load");
            }
        }

        // 2. 按请求顺序更新状态并生成分类结果
        let mut result = ToolLoadResult::default();
        for name in names {
            if !self.loaded.insert(name.clone()) {
                result.already_loaded_tools.push(name.clone());
            } else {
                self.loaded_order.push(name.clone());
                result.newly_loaded_tools.push(name.clone());
            }
        }
        Ok(result)
    }
}

#[derive(Default)]
struct ToolLoadResult {
    newly_loaded_tools: Vec<String>,
    already_loaded_tools: Vec<String>,
}

impl ToolLoadResult {
    /// 判断当前载入请求是否只命中了已经载入的工具。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 是否没有新增任何工具且存在已经载入的工具
    fn is_already_loaded_request(&self) -> bool {
        self.newly_loaded_tools.is_empty() && !self.already_loaded_tools.is_empty()
    }
}

#[cfg(test)]
#[path = "tool_visibility_batch_tests.rs"]
mod batch_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{self, ToolSpec};
    use serde_json::{json, Value};

    #[test]
    fn progressive_visibility_starts_with_base_and_gateway_tools() {
        let mut registry = test_registry();
        tools::register_progressive_loader(&mut registry, &wildcard_deferred());
        let visibility = ToolVisibility::new(wildcard_deferred());
        let names = definition_names(visibility.definitions(&registry));

        assert_eq!(names, ["read_file", tools::LOAD_NAME, tools::INVOKE_NAME]);
        assert!(visibility.is_visible("read_file"));
        assert!(!visibility.is_visible("web_search"));
    }

    #[test]
    fn anchored_standard_exposes_minimal_pair_then_resident_gateway() {
        let config = AppConfig::default();
        let mut registry = anchored_registry();
        let mut visibility = ToolVisibility::from_config_with_anchor(&config, true, false);
        let deferred = visibility.deferred_tools().to_vec();
        tools::register_progressive_loader(&mut registry, &deferred);

        assert_eq!(
            definition_names(visibility.definitions(&registry)),
            ["bash", "str_replace_editor"]
        );
        assert!(visibility.is_visible("bash"));
        assert!(!visibility.is_visible(tools::LOAD_NAME));
        assert!(visibility.is_visible("read_file"));

        assert!(visibility.promote_anchor());
        assert_eq!(
            definition_names(visibility.definitions(&registry)),
            [
                "bash",
                "str_replace_editor",
                tools::LOAD_NAME,
                tools::INVOKE_NAME,
            ]
        );
        assert!(visibility.is_visible(tools::LOAD_NAME));
        assert!(visibility.is_visible("write_file"));
    }

    #[test]
    fn progressive_visibility_keeps_definitions_fixed_after_loading() {
        let mut registry = test_registry();
        tools::register_progressive_loader(&mut registry, &wildcard_deferred());
        let mut visibility = ToolVisibility::new(wildcard_deferred());

        load_args(
            &mut visibility,
            &registry,
            r#"{"type":"tool","keywords":["web_search"]}"#,
        );
        let names = definition_names(visibility.definitions(&registry));

        assert_eq!(names, ["read_file", tools::LOAD_NAME, tools::INVOKE_NAME]);
        assert!(visibility.is_visible("web_search"));
        assert!(!visibility.is_visible("analyze_image"));
    }

    /// 验证显式延迟配置只隐藏指定工具。
    #[test]
    fn explicit_deferred_list_keeps_other_tools_native() {
        let deferred = vec!["web_search".to_string()];
        let mut registry = test_registry();
        tools::register_progressive_loader(&mut registry, &deferred);
        let visibility = ToolVisibility::new(deferred);
        let names = definition_names(visibility.definitions(&registry));

        assert_eq!(
            names,
            [
                "read_file",
                "analyze_image",
                tools::LOAD_NAME,
                tools::INVOKE_NAME,
            ]
        );
        assert!(!visibility.is_visible("web_search"));
        assert!(visibility.is_visible("analyze_image"));
    }

    /// 验证加载任意工具后供应商工具数组保持逐字稳定。
    #[test]
    fn progressive_visibility_never_changes_provider_definitions() {
        let deferred = vec!["deferred_first".to_string(), "deferred_second".to_string()];
        let mut registry = ToolRegistry::new();
        for name in ["read_file", "deferred_first", "grep", "deferred_second"] {
            registry.register(ToolSpec::new(
                name,
                "test",
                json!({"type":"object","properties":{},"additionalProperties":false}),
                |_| async { Ok("ok".to_string()) },
            ));
        }
        tools::register_progressive_loader(&mut registry, &deferred);
        let mut visibility = ToolVisibility::new(deferred);

        let initial_definitions = visibility.definitions(&registry);
        assert_eq!(
            definition_names(initial_definitions.clone()),
            ["read_file", "grep", tools::LOAD_NAME, tools::INVOKE_NAME]
        );
        let initial = serde_json::to_value(initial_definitions).unwrap();

        load_args(
            &mut visibility,
            &registry,
            r#"{"type":"tool","keywords":["deferred_first"]}"#,
        );
        let after_first = serde_json::to_value(visibility.definitions(&registry)).unwrap();
        assert_eq!(after_first, initial);

        load_args(
            &mut visibility,
            &registry,
            r#"{"type":"tool","keywords":["deferred_second"]}"#,
        );
        let after_second = serde_json::to_value(visibility.definitions(&registry)).unwrap();
        assert_eq!(after_second, initial);
    }

    #[test]
    fn progressive_visibility_reports_duplicate_tool_load() {
        let mut registry = test_registry();
        tools::register_progressive_loader(&mut registry, &wildcard_deferred());
        let mut visibility = ToolVisibility::new(wildcard_deferred());

        let first = load_args(
            &mut visibility,
            &registry,
            r#"{"type":"tool","keywords":["web_search"]}"#,
        );
        let second = load_args(
            &mut visibility,
            &registry,
            r#"{"type":"tool","keywords":["web_search"]}"#,
        );
        let first = serde_json::from_str::<Value>(&first).unwrap();
        let second = serde_json::from_str::<Value>(&second).unwrap();

        assert_eq!(first["already_loaded"], json!(false));
        assert_eq!(first["tools"][0]["name"], json!("web_search"));
        assert_eq!(
            first["tools"][0]["definition"]["function"]["name"],
            json!("web_search")
        );
        assert!(first["tools"][0]["definition"]["function"]["parameters"].is_object());
        assert_eq!(second["already_loaded"], json!(true));
        assert_eq!(second["tools"][0]["status"], json!("already_loaded"));
        assert!(second["instruction"]
            .as_str()
            .unwrap()
            .contains("Do not call load"));
    }

    #[test]
    fn progressive_visibility_keeps_loader_description_stable_after_loading() {
        let mut registry = test_registry();
        tools::register_progressive_loader(&mut registry, &wildcard_deferred());
        let mut visibility = ToolVisibility::new(wildcard_deferred());

        let initial = visibility
            .definitions(&registry)
            .into_iter()
            .find(|definition| definition.function.name == tools::LOAD_NAME)
            .unwrap()
            .function
            .description;

        load_args(
            &mut visibility,
            &registry,
            r#"{"type":"tool","keywords":["web_search"]}"#,
        );
        let definitions = visibility.definitions(&registry);
        let description = definitions
            .iter()
            .find(|definition| definition.function.name == tools::LOAD_NAME)
            .unwrap()
            .function
            .description
            .as_str();

        assert_eq!(description, initial.as_str());
        assert!(!description.contains("Already loaded tools"));
        assert!(description.contains("Available groups"));
        assert!(description.contains("web_search"));
        assert!(description.contains("web"));
        assert!(description.contains("analyze_image"));
    }

    /// load 描述只反映当前 registry 中的可加载工具，因此会随 agent enabled_tools 过滤结果变化。
    #[test]
    fn loader_description_follows_agent_filtered_registry() {
        let mut registry = ToolRegistry::new();
        registry.register(ToolSpec::new(
            "read_file",
            "Read a file.",
            json!({"type":"object","properties":{},"additionalProperties":false}),
            |_| async { Ok("ok".to_string()) },
        ));
        registry.register(ToolSpec::new(
            "web_search",
            "Search the web.",
            json!({"type":"object","properties":{},"additionalProperties":false}),
            |_| async { Ok("ok".to_string()) },
        ));
        tools::register_progressive_loader(&mut registry, &wildcard_deferred());
        let visibility = ToolVisibility::new(wildcard_deferred());
        let description = visibility
            .definitions(&registry)
            .into_iter()
            .find(|definition| definition.function.name == tools::LOAD_NAME)
            .unwrap()
            .function
            .description;

        assert!(description.contains("web_search"));
        assert!(description.contains("Available groups"));
        assert!(!description.contains("read_file"));
        assert!(!description.contains("analyze_image"));
        assert!(!description.contains("deep_diagnose"));
    }

    #[test]
    fn progressive_loader_loads_skill_document() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let skill_dir = paths.skills_dir.join("gpu-passthrough");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: gpu-passthrough\ndescription: GPU switching\n---\n\nUse `gpustoggle --status`.",
        )
        .unwrap();
        let registry = test_registry();
        let config = AppConfig::default();
        let mut visibility = ToolVisibility::new(wildcard_deferred());

        let output = visibility
            .load_from_arguments(
                &registry,
                r#"{"type":"skill","keywords":["gpu-passthrough"]}"#,
                &config,
                &paths,
            )
            .unwrap();

        let output = serde_json::from_str::<serde_json::Value>(&output).unwrap();
        assert!(output["skills"].is_array());
        assert!(output["skills"][0]["content"]
            .as_str()
            .unwrap()
            .contains("<loaded-skill"));
        assert!(output.to_string().contains("gpu-passthrough"));
        assert!(output.to_string().contains("gpustoggle --status"));
    }

    #[test]
    fn progressive_loader_rejects_skill_when_disabled() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let registry = test_registry();
        let mut config = AppConfig::default();
        config.skills.enabled = false;
        let mut visibility = ToolVisibility::new(wildcard_deferred());

        let err = visibility
            .load_from_arguments(
                &registry,
                r#"{"type":"skill","keywords":["yce"]}"#,
                &config,
                &paths,
            )
            .unwrap_err();

        assert!(err.to_string().contains("skill loading is disabled"));
    }

    #[test]
    fn progressive_visibility_restores_loaded_tools() {
        let mut registry = test_registry();
        tools::register_progressive_loader(&mut registry, &wildcard_deferred());
        let mut visibility = ToolVisibility::new(wildcard_deferred());

        visibility.restore_loaded_tools(
            &registry,
            &[
                "web_search".to_string(),
                "unknown_tool".to_string(),
                "read_file".to_string(),
            ],
        );
        let names = definition_names(visibility.definitions(&registry));

        assert_eq!(names, ["read_file", tools::LOAD_NAME, tools::INVOKE_NAME]);
        assert!(visibility.is_visible("web_search"));
        assert!(visibility.is_visible("read_file"));
        assert!(!visibility.is_visible("unknown_tool"));
        assert_eq!(
            visibility.loaded_tool_names(),
            vec!["web_search".to_string()]
        );
    }

    /// 构造「非基础工具一律需要 load」的延迟集合。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 只含通配符的延迟工具集合
    fn wildcard_deferred() -> Vec<String> {
        vec![crate::config::DEFERRED_ALL_NON_BASE.to_string()]
    }

    fn test_registry() -> ToolRegistry {
        let mut registry = ToolRegistry::new();
        registry.register(ToolSpec::new(
            "read_file",
            "Read a file.",
            json!({"type":"object","properties":{},"additionalProperties":false}),
            |_| async { Ok("ok".to_string()) },
        ));
        registry.register(ToolSpec::new(
            "web_search",
            "Search the web.",
            json!({"type":"object","properties":{},"additionalProperties":false}),
            |_| async { Ok("ok".to_string()) },
        ));
        registry.register(ToolSpec::new(
            "analyze_image",
            "Analyze an image.",
            json!({"type":"object","properties":{},"additionalProperties":false}),
            |_| async { Ok("ok".to_string()) },
        ));
        registry
    }

    fn anchored_registry() -> ToolRegistry {
        let mut registry = ToolRegistry::new();
        for name in ["run_command", "str_replace", "read_file", "write_file"] {
            registry.register(ToolSpec::new(
                name,
                "test",
                json!({"type":"object","properties":{},"additionalProperties":false}),
                |_| async { Ok("ok".to_string()) },
            ));
        }
        registry
    }

    fn load_args(
        visibility: &mut ToolVisibility,
        registry: &ToolRegistry,
        arguments: &str,
    ) -> String {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let config = AppConfig::default();
        visibility
            .load_from_arguments(registry, arguments, &config, &paths)
            .unwrap()
    }

    fn test_paths(root: &std::path::Path) -> SaiPaths {
        SaiPaths {
            config_dir: root.join("config"),
            config_file: root.join("config/config.jsonc"),
            secrets_file: root.join("config/secrets.jsonc"),
            skills_dir: root.join("config/skills"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            state_dir: root.join("state"),
            pictures_dir: root.join("pictures"),
            fish_hook_file: root.join("fish/sai.fish"),
            bash_hook_file: root.join("shell/bash-hook.sh"),
            zsh_hook_file: root.join("shell/zsh-hook.zsh"),
            powershell_hook_file: root.join("shell/powershell-hook.ps1"),
        }
    }

    fn definition_names(definitions: Vec<ToolDefinition>) -> Vec<String> {
        definitions
            .into_iter()
            .map(|definition| definition.function.name)
            .collect()
    }
}
