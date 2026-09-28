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
    /// 预选只宣布过名称和描述、尚未 load 的工具
    pub(super) announced_tools: BTreeSet<String>,
    /// 预选只宣布过名称和描述、尚未 load 的 skill
    pub(super) announced_skills: BTreeSet<String>,
    /// DeepSeek Anchored Standard 是否控制当前会话的工具目录。
    anchor_enabled: bool,
    /// false 表示请求 #1 尚未产生持久 assistant/tool 信号。
    anchor_promoted: bool,
    /// 是否由 Jev 决定额外工具与 skill 的暴露
    jev_routing: bool,
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
            announced_tools: BTreeSet::new(),
            announced_skills: BTreeSet::new(),
            anchor_enabled: false,
            anchor_promoted: false,
            jev_routing: false,
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
        if config.jev.routing.enabled {
            return Self::with_jev_routing(config.agent_deferred_tools());
        }
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
            // 锚定模式自带工具目录控制，Jev 暴露决策让位
            visibility.jev_routing = false;
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
            if registry.contains(name)
                && self.is_loadable_tool(name)
                && self.loaded.insert(name.clone())
            {
                self.loaded_order.push(name.clone());
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

#[path = "jev_visibility.rs"]
mod jev_visibility;

#[cfg(test)]
#[path = "tool_visibility_batch_tests.rs"]
mod batch_tests;

#[cfg(test)]
#[path = "tool_visibility_tests.rs"]
mod tests;
