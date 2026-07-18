use crate::config::AppConfig;
use crate::llm::ToolDefinition;
use crate::paths::SaiPaths;
use crate::tools::{self, ToolRegistry};
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct ToolVisibility {
    progressive: bool,
    loaded: BTreeSet<String>,
}

impl ToolVisibility {
    /// 创建工具可见性状态。
    ///
    /// 参数:
    /// - `progressive`: 是否启用渐进式工具加载
    ///
    /// 返回:
    /// - 新的工具可见性状态
    pub(crate) fn new(progressive: bool) -> Self {
        Self {
            progressive,
            loaded: BTreeSet::new(),
        }
    }

    /// 计算当前应暴露给模型的工具定义。
    ///
    /// 参数:
    /// - `registry`: 完整工具注册表
    ///
    /// 返回:
    /// - 当前可见的工具定义列表
    pub(crate) fn definitions(&self, registry: &ToolRegistry) -> Vec<ToolDefinition> {
        if !self.progressive {
            return registry.definitions();
        }
        let names = registry
            .tool_infos()
            .into_iter()
            .filter(|info| self.is_visible(&info.name))
            .map(|info| info.name)
            .collect::<BTreeSet<_>>();
        let mut definitions = registry.definitions_for_names(&names);
        for definition in &mut definitions {
            if definition.function.name == tools::LOAD_NAME {
                definition.function.description =
                    tools::progressive::loader_description(registry, &self.loaded);
            }
        }
        definitions
    }

    /// 判断工具当前是否允许被模型调用。
    ///
    /// 参数:
    /// - `name`: 工具名称
    ///
    /// 返回:
    /// - 当前是否可见并允许调用
    pub(crate) fn is_visible(&self, name: &str) -> bool {
        !self.progressive || tools::progressive::is_initial_tool(name) || self.loaded.contains(name)
    }

    /// 判断当前工具调用是否为加载工具调用。
    ///
    /// 参数:
    /// - `name`: 工具名称
    ///
    /// 返回:
    /// - 是否为 `load`
    pub(crate) fn is_loader_call(&self, name: &str) -> bool {
        self.progressive && name == tools::LOAD_NAME
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
        if !self.progressive {
            return;
        }
        for name in names {
            if registry.contains(name) && !tools::progressive::is_initial_tool(name) {
                self.loaded.insert(name.clone());
            }
        }
    }

    /// 获取已经额外加载的工具名称。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 已加载工具名称列表
    pub(crate) fn loaded_tool_names(&self) -> Vec<String> {
        self.loaded.iter().cloned().collect()
    }

    /// 生成当前已经载入工具的系统提示。
    ///
    /// 参数:
    /// - `registry`: 完整工具注册表
    ///
    /// 返回:
    /// - 已载入工具提示，未启用渐进式加载或尚未载入工具时返回空
    pub(crate) fn loaded_context_prompt(&self, registry: &ToolRegistry) -> Option<String> {
        if !self.progressive || self.loaded.is_empty() {
            return None;
        }
        let mut text = String::from("<loaded_tools>\n");
        text.push_str("The following tools are already loaded in this conversation. Do not call load for them again; call the loaded tool directly. If one of these tools returns an error, treat it as an execution or workflow error, not as a loading error.\n");
        text.push_str(&format!(
            "Loaded tools: {}\n",
            self.loaded_tool_names().join(", ")
        ));
        let groups = self.loaded_group_names(registry);
        if !groups.is_empty() {
            text.push_str(&format!("Loaded groups: {}\n", groups.join(", ")));
        }
        text.push_str("</loaded_tools>");
        Some(text)
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
        let args = if arguments.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str::<Value>(arguments)?
        };
        let tool_name = string_arg(&args, "tool_name");
        let tool_names = string_array_arg(&args, "tool_names")?;
        let group_name = string_arg(&args, "group_name");
        let skill_name = string_arg(&args, "skill_name");
        let requested_count = [
            tool_name.is_some(),
            tool_names.is_some(),
            group_name.is_some(),
            skill_name.is_some(),
        ]
        .into_iter()
        .filter(|selected| *selected)
        .count();
        if requested_count != 1 {
            bail!("provide exactly one of tool_name, tool_names, group_name, or skill_name");
        }
        if let Some(skill_name) = skill_name {
            if !config.skills.enabled {
                bail!("skill loading is disabled");
            }
            return tools::load_installed_skill(&skill_name, config, paths);
        }
        let (requested_tool, requested_tools, requested_group, result) =
            if let Some(tool_name) = tool_name {
                let result = self.load_tool(registry, &tool_name)?;
                (Some(tool_name), None, None, result)
            } else if let Some(tool_names) = tool_names {
                let result = self.load_tools(registry, &tool_names)?;
                (None, Some(tool_names), None, result)
            } else {
                let group_name = group_name.unwrap();
                let result = self.load_group(registry, &group_name)?;
                (None, None, Some(group_name), result)
            };
        let already_loaded = result.is_already_loaded_request();
        let instruction = if already_loaded {
            "This request only targeted tools that were already loaded. Do not call load for this target again; call the tool directly."
        } else {
            "The requested tools are now available. Call the loaded tool directly; do not reload it before use."
        };
        Ok(serde_json::to_string_pretty(&json!({
            "ok": true,
            "requested_tool": requested_tool,
            "requested_tools": requested_tools,
            "requested_group": requested_group,
            "already_loaded": already_loaded,
            "newly_loaded_tools": result.newly_loaded_tools,
            "already_loaded_tools": result.already_loaded_tools,
            "currently_loaded_tools": self.loaded_tool_names(),
            "loaded_groups": self.loaded_group_names(registry),
            "visible_tools": self.visible_tool_names(registry),
            "instruction": instruction,
        }))?)
    }

    /// 加载单个工具。
    ///
    /// 参数:
    /// - `registry`: 完整工具注册表
    /// - `name`: 要加载的工具名称
    ///
    /// 返回:
    /// - 本次请求加载的工具名称列表
    fn load_tool(&mut self, registry: &ToolRegistry, name: &str) -> Result<ToolLoadResult> {
        self.load_tools(registry, &[name.to_string()])
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
        }

        // 2. 按请求顺序更新状态并生成分类结果
        let mut result = ToolLoadResult::default();
        for name in names {
            if tools::progressive::is_initial_tool(name) || !self.loaded.insert(name.clone()) {
                result.already_loaded_tools.push(name.clone());
            } else {
                result.newly_loaded_tools.push(name.clone());
            }
        }
        Ok(result)
    }

    /// 加载一个用途分组下的所有工具。
    ///
    /// 参数:
    /// - `registry`: 完整工具注册表
    /// - `group`: 要加载的分组名称
    ///
    /// 返回:
    /// - 本次请求加载的工具名称列表
    fn load_group(&mut self, registry: &ToolRegistry, group: &str) -> Result<ToolLoadResult> {
        let names = registry
            .tool_infos()
            .into_iter()
            .filter(|info| {
                !tools::progressive::is_initial_tool(&info.name)
                    && tools::progressive::tool_group(&info.name) == group
            })
            .map(|info| info.name)
            .collect::<Vec<_>>();
        if names.is_empty() {
            bail!("unknown or empty tool group: {group}");
        }
        let mut result = ToolLoadResult::default();
        for name in &names {
            if self.loaded.insert(name.clone()) {
                result.newly_loaded_tools.push(name.clone());
            } else {
                result.already_loaded_tools.push(name.clone());
            }
        }
        Ok(result)
    }

    /// 获取已经完整载入的分组名称。
    ///
    /// 参数:
    /// - `registry`: 完整工具注册表
    ///
    /// 返回:
    /// - 已完整载入的分组名称列表
    fn loaded_group_names(&self, registry: &ToolRegistry) -> Vec<String> {
        let mut groups = BTreeMap::<&'static str, (usize, usize)>::new();
        for info in registry.tool_infos() {
            if tools::progressive::is_initial_tool(&info.name) {
                continue;
            }
            let group = tools::progressive::tool_group(&info.name);
            let entry = groups.entry(group).or_default();
            entry.0 += 1;
            if self.loaded.contains(&info.name) {
                entry.1 += 1;
            }
        }
        groups
            .into_iter()
            .filter_map(|(group, (total, loaded))| {
                (total > 0 && total == loaded).then_some(group.to_string())
            })
            .collect()
    }

    /// 获取当前可见工具名称。
    ///
    /// 参数:
    /// - `registry`: 完整工具注册表
    ///
    /// 返回:
    /// - 当前可见工具名称列表
    fn visible_tool_names(&self, registry: &ToolRegistry) -> Vec<String> {
        registry
            .tool_infos()
            .into_iter()
            .filter(|info| self.is_visible(&info.name))
            .map(|info| info.name)
            .collect()
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

/// 从 JSON 参数中读取非空字符串。
///
/// 参数:
/// - `args`: JSON 参数对象
/// - `name`: 字段名
///
/// 返回:
/// - 字段存在且非空时返回字符串
fn string_arg(args: &Value, name: &str) -> Option<String> {
    args.get(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// 从 JSON 参数中读取非空字符串数组，并按首次出现顺序去重。
///
/// 参数:
/// - `args`: JSON 参数对象
/// - `name`: 字段名
///
/// 返回:
/// - 字段不存在时返回空，字段合法时返回去重后的字符串列表
fn string_array_arg(args: &Value, name: &str) -> Result<Option<Vec<String>>> {
    let Some(value) = args.get(name) else {
        return Ok(None);
    };
    let Some(values) = value.as_array() else {
        bail!("{name} must be a non-empty array of strings");
    };
    if values.is_empty() {
        bail!("{name} must be a non-empty array of strings");
    }
    let mut unique = BTreeSet::new();
    let mut result = Vec::new();
    for value in values {
        let Some(value) = value.as_str().map(str::trim) else {
            bail!("{name} must contain only non-empty strings");
        };
        if value.is_empty() {
            bail!("{name} must contain only non-empty strings");
        }
        if unique.insert(value.to_string()) {
            result.push(value.to_string());
        }
    }
    Ok(Some(result))
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
    fn progressive_visibility_starts_with_base_and_loader() {
        let mut registry = test_registry();
        tools::register_progressive_loader(&mut registry);
        let visibility = ToolVisibility::new(true);
        let names = definition_names(visibility.definitions(&registry));

        assert!(names.contains(&"read_file".to_string()));
        assert!(names.contains(&tools::LOAD_NAME.to_string()));
        assert!(!names.contains(&"web_search".to_string()));
    }

    #[test]
    fn progressive_visibility_loads_group() {
        let mut registry = test_registry();
        tools::register_progressive_loader(&mut registry);
        let mut visibility = ToolVisibility::new(true);

        load_args(&mut visibility, &registry, r#"{"group_name":"web"}"#);
        let names = definition_names(visibility.definitions(&registry));

        assert!(names.contains(&"web_search".to_string()));
        assert!(!names.contains(&"analyze_image".to_string()));
    }

    #[test]
    fn progressive_visibility_reports_duplicate_group_load() {
        let mut registry = test_registry();
        tools::register_progressive_loader(&mut registry);
        let mut visibility = ToolVisibility::new(true);

        let first = load_args(&mut visibility, &registry, r#"{"group_name":"web"}"#);
        let second = load_args(&mut visibility, &registry, r#"{"group_name":"web"}"#);
        let first = serde_json::from_str::<Value>(&first).unwrap();
        let second = serde_json::from_str::<Value>(&second).unwrap();

        assert_eq!(first["already_loaded"], json!(false));
        assert_eq!(first["newly_loaded_tools"], json!(["web_search"]));
        assert_eq!(second["already_loaded"], json!(true));
        assert_eq!(second["newly_loaded_tools"], json!([]));
        assert_eq!(second["already_loaded_tools"], json!(["web_search"]));
        assert!(second["instruction"]
            .as_str()
            .unwrap()
            .contains("Do not call load"));
    }

    #[test]
    fn progressive_visibility_updates_loader_description() {
        let mut registry = test_registry();
        tools::register_progressive_loader(&mut registry);
        let mut visibility = ToolVisibility::new(true);

        load_args(&mut visibility, &registry, r#"{"group_name":"web"}"#);
        let definitions = visibility.definitions(&registry);
        let description = definitions
            .iter()
            .find(|definition| definition.function.name == tools::LOAD_NAME)
            .unwrap()
            .function
            .description
            .as_str();

        assert!(description.contains("Already loaded tools"));
        assert!(description.contains("web_search"));
        assert!(description.contains("Already loaded groups"));
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
        tools::register_progressive_loader(&mut registry);
        let visibility = ToolVisibility::new(true);
        let description = visibility
            .definitions(&registry)
            .into_iter()
            .find(|definition| definition.function.name == tools::LOAD_NAME)
            .unwrap()
            .function
            .description;

        assert!(description.contains("web_search"));
        assert!(description.contains("Available groups"));
        assert!(!description.contains("analyze_image"));
        assert!(!description.contains("deep_research"));
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
        let mut visibility = ToolVisibility::new(true);

        let output = visibility
            .load_from_arguments(
                &registry,
                r#"{"skill_name":"gpu-passthrough"}"#,
                &config,
                &paths,
            )
            .unwrap();

        assert!(output.contains("<loaded-skill"));
        assert!(output.contains("gpu-passthrough"));
        assert!(output.contains("gpustoggle --status"));
    }

    #[test]
    fn progressive_loader_rejects_skill_when_disabled() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let registry = test_registry();
        let mut config = AppConfig::default();
        config.skills.enabled = false;
        let mut visibility = ToolVisibility::new(true);

        let err = visibility
            .load_from_arguments(&registry, r#"{"skill_name":"yce"}"#, &config, &paths)
            .unwrap_err();

        assert!(err.to_string().contains("skill loading is disabled"));
    }

    #[test]
    fn progressive_visibility_restores_loaded_tools() {
        let mut registry = test_registry();
        tools::register_progressive_loader(&mut registry);
        let mut visibility = ToolVisibility::new(true);

        visibility.restore_loaded_tools(
            &registry,
            &[
                "web_search".to_string(),
                "unknown_tool".to_string(),
                "read_file".to_string(),
            ],
        );
        let names = definition_names(visibility.definitions(&registry));

        assert!(names.contains(&"web_search".to_string()));
        assert!(names.contains(&"read_file".to_string()));
        assert!(!names.contains(&"unknown_tool".to_string()));
        assert_eq!(
            visibility.loaded_tool_names(),
            vec!["web_search".to_string()]
        );
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
