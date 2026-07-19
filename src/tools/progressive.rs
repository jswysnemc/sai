use super::groups::{group_description, group_for_tool, is_base_tool};
use super::{ToolPermission, ToolRegistry, ToolSpec};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const LOAD_NAME: &str = "load";

/// 注册渐进式工具加载器。
///
/// 参数:
/// - `registry`: 已注册完整工具处理器的工具注册表
///
/// 返回:
/// - 无
pub(crate) fn register_loader(registry: &mut ToolRegistry) {
    let description = loader_description(registry, &BTreeSet::new());
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
}

/// 判断工具是否应在渐进式模式启动时默认可见。
///
/// 参数:
/// - `name`: 工具名称
///
/// 返回:
/// - 是否为默认可见工具
pub(crate) fn is_initial_tool(name: &str) -> bool {
    name == LOAD_NAME || is_base_tool(name)
}

/// 获取工具所属用途分组。
///
/// 参数:
/// - `name`: 工具名称
///
/// 返回:
/// - 用途分组名称
pub(crate) fn tool_group(name: &str) -> &'static str {
    group_for_tool(name)
}

/// 生成加载工具描述。
///
/// 描述内容只来自当前传入的 `registry`，因此当 agent 配置按
/// `enabled_tools` 过滤注册表后，`load` 只会列出该 agent 实际可加载的工具与分组。
///
/// 参数:
/// - `registry`: 当前会话可见/可注册的工具注册表（可已按 agent 配置过滤）
/// - `loaded`: 本会话已额外加载的工具名
///
/// 返回:
/// - 包含可加载工具名和分组的工具描述
pub(crate) fn loader_description(registry: &ToolRegistry, loaded: &BTreeSet<String>) -> String {
    let mut groups: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
    let mut group_totals = BTreeMap::<&'static str, usize>::new();
    let mut group_loaded = BTreeMap::<&'static str, usize>::new();
    let mut loaded_tools = Vec::new();
    for info in registry.tool_infos() {
        if is_initial_tool(&info.name) {
            continue;
        }
        let group = group_for_tool(&info.name);
        *group_totals.entry(group).or_default() += 1;
        if loaded.contains(&info.name) {
            *group_loaded.entry(group).or_default() += 1;
            loaded_tools.push(info.name);
            continue;
        }
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
        "Load additional tools or full skill documents before using them. Set type to tool or skill and pass exact names in the keywords array, including when loading one item. Multiple tools or skills can be loaded in one call. Tools listed inside a group can be loaded individually. Initial tools are limited to base tools and this loader. Do not call load for already loaded tools; call the loaded tool directly. If a loaded tool returns an error, treat it as a tool execution or workflow error, not as a loading error.\n",
    );
    if !loaded_tools.is_empty() {
        let loaded_groups = fully_loaded_groups(&group_totals, &group_loaded);
        text.push_str("\nAlready loaded tools:\n");
        text.push_str(&format!("- {}\n", loaded_tools.join(", ")));
        if !loaded_groups.is_empty() {
            text.push_str("Already loaded groups:\n");
            text.push_str(&format!("- {}\n", loaded_groups.join(", ")));
        }
    }
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

/// 计算已经完整载入的工具分组。
///
/// 参数:
/// - `group_totals`: 每个分组的工具总数
/// - `group_loaded`: 每个分组已经载入的工具数量
///
/// 返回:
/// - 已经完整载入的分组名称
fn fully_loaded_groups(
    group_totals: &BTreeMap<&'static str, usize>,
    group_loaded: &BTreeMap<&'static str, usize>,
) -> Vec<&'static str> {
    group_totals
        .iter()
        .filter_map(|(group, total)| {
            let loaded = group_loaded.get(group).copied().unwrap_or_default();
            (*total > 0 && loaded == *total).then_some(*group)
        })
        .collect()
}
