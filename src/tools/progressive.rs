use super::groups::{group_description, group_for_tool, is_base_tool};
use super::{ToolPermission, ToolRegistry, ToolSpec};
use crate::config::DEFERRED_ALL_NON_BASE;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const LOAD_NAME: &str = "load";
pub(crate) const INVOKE_NAME: &str = "invoke_tool";
/// Anchored Standard 的 resident 目录只常驻 dsh 适配工具和渐进网关。
pub(crate) const DEFERRED_ALL_EXCEPT_ANCHOR_BOOTSTRAP: &str = "**anchored-standard**";

/// 注册渐进式工具加载器。
///
/// 参数:
/// - `registry`: 已注册完整工具处理器的工具注册表
/// - `deferred`: 当前 Agent 需要 load 才暴露的工具名，可含通配符
///
/// 返回:
/// - 无
pub(crate) fn register_loader(registry: &mut ToolRegistry, deferred: &[String]) {
    let description = loader_description(registry, deferred);
    registry.register(ToolSpec::new(
        LOAD_NAME,
        description,
        json!({
            "type": "object",
            "properties": {
                "type": {
                    "type": "string",
                    "enum": ["tool", "skill"],
                    "description": "Resource type to load."
                },
                "keywords": {
                    "type": "array",
                    "items": {
                        "type": "string"
                    },
                    "minItems": 1,
                    "uniqueItems": true,
                    "description": "Exact tool or installed skill names to load. Always pass an array, including for one item."
                }
            },
            "required": ["type", "keywords"],
            "additionalProperties": false
        }),
        |_| async move {
            Ok("工具加载请求已收到。后续工具可见性由对话运行时更新。".to_string())
        },
    ));
    if !deferred.is_empty() {
        register_invoker(registry);
    }
}

/// 注册统一工具调用外壳。
///
/// 参数:
/// - `registry`: 已注册完整工具处理器的工具注册表
///
/// 返回:
/// - 无
fn register_invoker(registry: &mut ToolRegistry) {
    registry.register(ToolSpec::new(
        INVOKE_NAME,
        "Invoke a tool whose full schema was returned by load. Pass the exact loaded tool name and an arguments object matching that schema. Concrete arguments are validated locally before the real tool enters its normal permission, audit, hook, progress, and execution pipeline.",
        json!({
            "type": "object",
            "properties": {
                "tool_name": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Exact name of a tool previously loaded with load."
                },
                "arguments": {
                    "type": "object",
                    "description": "Arguments matching the full schema returned by load."
                }
            },
            "required": ["tool_name", "arguments"],
            "additionalProperties": false
        }),
        |_| async move {
            Ok("统一工具调用请求已收到。真实工具由对话运行时执行。".to_string())
        },
    ));
}

/// 计算当前应暴露给模型的工具名称集合。
///
/// 参数:
/// - `registry`: 完整工具注册表
/// - `deferred`: 当前 Agent 需要 load 才暴露的工具名，可含通配符
/// 返回:
/// - 非渐进模式返回完整注册表；渐进模式只返回固定网关
pub(crate) fn visible_tool_names(registry: &ToolRegistry, deferred: &[String]) -> BTreeSet<String> {
    registry
        .tool_infos()
        .into_iter()
        .filter(|info| !is_deferred_tool(&info.name, deferred))
        .map(|info| info.name)
        .collect()
}

/// 判断工具是否必须经过渐进网关加载。
///
/// 基础集合只是默认值，不是不可覆盖的策略：显式点名的工具一律按需 load，
/// 基础工具也不例外，否则用户在配置界面上把它切成"按需"却始终不生效。
/// 通配符才回落到基础集合，因为全量开放的 Agent 无法穷举工具名。
///
/// 参数:
/// - `name`: 工具名称
/// - `deferred`: 当前 Agent 的延迟工具配置
///
/// 返回:
/// - 工具是否需要先调用 load
pub(crate) fn is_deferred_tool(name: &str, deferred: &[String]) -> bool {
    if name == LOAD_NAME || name == INVOKE_NAME {
        return false;
    }
    if deferred
        .iter()
        .any(|configured| configured == DEFERRED_ALL_EXCEPT_ANCHOR_BOOTSTRAP)
    {
        // dsh 的常驻工具由 Agent 适配层提供；注册表中的 sai 本地定义全部保持延迟。
        return true;
    }
    // 1. 用户点名优先于内置基础集合：配置界面上选了"按需"就得真的按需
    if deferred.iter().any(|configured| configured == name) {
        return true;
    }
    if is_base_tool(name) {
        return false;
    }
    deferred
        .iter()
        .any(|configured| configured == DEFERRED_ALL_NON_BASE)
}

/// 生成加载工具描述。
///
/// 描述内容只来自当前传入的 `registry` 与该 Agent 的延迟集合，因此当 agent 配置按
/// `enabled_tools` 过滤注册表后，`load` 只会列出该 agent 实际可加载的工具与分组。
///
/// 参数:
/// - `registry`: 当前会话可见/可注册的工具注册表（可已按 agent 配置过滤）
/// - `deferred`: 当前 Agent 需要 load 才暴露的工具名，可含通配符
/// 返回:
/// - 包含可加载工具名和分组的工具描述
pub(crate) fn loader_description(registry: &ToolRegistry, deferred: &[String]) -> String {
    let mut groups: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
    for info in registry.tool_infos() {
        // 1. 只列出按配置需要延迟的工具，基础工具和其他原生工具不进入 load
        if !is_deferred_tool(&info.name, deferred) {
            continue;
        }
        let group = group_for_tool(&info.name);
        let permission = match info.permission {
            ToolPermission::ReadOnly => "read",
            ToolPermission::Writes => "write",
        };
        let summary = info
            .description
            .split(['.', '。'])
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("no description");
        groups
            .entry(group)
            .or_default()
            .push(format!("{} ({permission}) - {summary}", info.name));
    }
    let mut text = String::from(
        "Load deferred tool schemas or skill documents before using them. Only tools listed below require load; tools already present as native definitions must be called directly. Set type to tool or skill and pass exact names in the keywords array, including when loading one item. Multiple resources can be loaded in one call. After loading a deferred tool, call invoke_tool with its exact name and arguments matching the returned schema. Do not emit a direct call to that deferred tool. Do not reload an already loaded tool unless its schema is no longer present in the conversation.\n",
    );
    text.push_str("\nAvailable groups:\n");
    if groups.is_empty() {
        text.push_str("- none. All additional tools are already loaded.\n");
        return text;
    }
    for (group, names) in groups {
        text.push_str(&format!(
            "- {group}: {}. Tools: {}\n",
            group_description(group),
            names.join(", ")
        ));
    }
    text
}
