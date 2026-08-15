use crate::llm::ToolCall;
use crate::tools::ToolRegistry;

/// 工具执行前置判定结果。
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum ToolGate {
    /// 允许继续走权限审计与实际执行
    Proceed,
    /// 判定失败，把这段文本作为工具错误回传给模型
    Reject(String),
}

/// 在权限审计和工具执行之前完成可恢复的参数与可见性检查。
///
/// 这些检查此前散落在权限审计链路里并用 `?` 冒泡，导致模型幻觉的工具名或截断的
/// JSON 参数会直接终止整轮对话，模型既拿不到错误也没有重试机会。改为返回
/// `Reject` 后统一按 `tool error:` 回传，模型可以在同一轮内自行纠正。
///
/// 参数:
/// - `registry`: 当前会话工具注册表
/// - `visibility`: 渐进式加载可见性状态
/// - `call`: 待执行的工具调用
/// - `used_tools`: 本轮已经执行过的工具名
///
/// 返回:
/// - 允许继续执行，或需要回传给模型的错误说明
pub(crate) fn evaluate_tool_gate(
    registry: &ToolRegistry,
    visibility: &super::tool_visibility::ToolVisibility,
    call: &ToolCall,
    used_tools: &[String],
) -> ToolGate {
    let name = call.function.name.as_str();
    // 1. load 走可见性专用分支，不做注册表存在性判定，但参数仍需可解析
    if visibility.is_loader_call(name) {
        return match registry.check_arguments(&call.function.arguments) {
            Ok(()) => ToolGate::Proceed,
            Err(err) => ToolGate::Reject(invalid_arguments_notice(name, &err)),
        };
    }
    // 2. 工具名不存在：模型幻觉或协议别名拼错，给出可用工具提示
    if !registry.resolves(name) {
        return ToolGate::Reject(unknown_tool_notice(registry, visibility, name));
    }
    // 3. 工具存在但当前不可见：提示先调用 load
    if !visibility.is_visible(name) {
        return ToolGate::Reject(format!(
            "tool error: tool {name} is not loaded in the current visible tool set; call load with type=tool and a keywords array first. If this tool was loaded in a previous conversation, the loaded-tool session state was reset or is unavailable."
        ));
    }
    // 4. 参数必须满足真实工具 Schema，统一外壳不能降低具体调用的校验强度
    if let Err(err) = registry.validate_arguments(name, &call.function.arguments) {
        return ToolGate::Reject(invalid_arguments_notice(name, &err));
    }
    // 5. AUR 安装与审查必须分轮，避免同轮内跳过用户确认
    if name == "install_aur_package" && used_tools.iter().any(|used| used == "review_aur_package") {
        return ToolGate::Reject(
            "tool error: install_aur_package cannot run in the same turn as review_aur_package. This is a workflow confirmation error, not a tool loading error. Do not call load again; ask the user to confirm installation in a new turn first.".to_string(),
        );
    }
    ToolGate::Proceed
}

/// 构造参数解析失败的错误说明。
///
/// 参数:
/// - `name`: 工具名
/// - `error`: 参数解析失败原因
///
/// 返回:
/// - 回传给模型的错误说明
fn invalid_arguments_notice(name: &str, error: &anyhow::Error) -> String {
    format!(
        "tool error: invalid arguments for {name}: {error:#}. Arguments must be a single JSON object matching the tool schema; remove unexpected fields, correct invalid values, and reissue the call."
    )
}

/// 构造未知工具名的错误说明。
///
/// 渐进式加载下未知名称可能只是尚未 load，两种情况的处置方式不同，需要分别说明。
///
/// 参数:
/// - `registry`: 当前会话工具注册表
/// - `visibility`: 渐进式加载可见性状态
/// - `name`: 模型给出的工具名
///
/// 返回:
/// - 回传给模型的错误说明
fn unknown_tool_notice(
    registry: &ToolRegistry,
    visibility: &super::tool_visibility::ToolVisibility,
    name: &str,
) -> String {
    if visibility.is_progressive() {
        return format!(
            "tool error: unknown tool: {name}. It is not registered in this session. Call load with type=tool and a keywords array to discover available tools, or pick a different tool; do not retry this name."
        );
    }
    let available = available_tool_names(registry, visibility);
    if available.is_empty() {
        return format!(
            "tool error: unknown tool: {name}. No tools are available in this session."
        );
    }
    format!(
        "tool error: unknown tool: {name}. Available tools: {available}. Pick one of these; do not retry this name."
    )
}

/// 列出当前可直接调用的工具名。
///
/// 参数:
/// - `registry`: 当前会话工具注册表
/// - `visibility`: 渐进式加载可见性状态
///
/// 返回:
/// - 逗号分隔的工具名；超过上限时截断并标注剩余数量
fn available_tool_names(
    registry: &ToolRegistry,
    visibility: &super::tool_visibility::ToolVisibility,
) -> String {
    const MAX_LISTED_TOOLS: usize = 40;
    let names = registry
        .tool_infos()
        .into_iter()
        .map(|info| info.name)
        .filter(|name| visibility.is_visible(name))
        .collect::<Vec<_>>();
    if names.len() <= MAX_LISTED_TOOLS {
        return names.join(", ");
    }
    let remaining = names.len() - MAX_LISTED_TOOLS;
    format!(
        "{}, ... ({remaining} more)",
        names
            .into_iter()
            .take(MAX_LISTED_TOOLS)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// 判断工具输出是否为错误结果。
///
/// 参数:
/// - `output`: 工具输出文本
///
/// 返回:
/// - 以 `tool error:` 开头时为 true
pub(super) fn is_tool_error_output(output: &str) -> bool {
    output.starts_with("tool error:")
}

/// 构造工具执行失败的统一错误文本。
///
/// 参数:
/// - `error`: 底层错误
///
/// 返回:
/// - 回传给模型的错误说明
pub(super) fn tool_error_output(error: &anyhow::Error) -> String {
    format!("tool error: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{ToolCall, ToolCallFunction};
    use crate::tools::{empty_parameters, ToolSpec};

    /// 构造只含一个工具的测试注册表。
    ///
    /// 参数:
    /// - `name`: 工具名
    ///
    /// 返回:
    /// - 测试注册表
    fn registry_with(name: &'static str) -> ToolRegistry {
        let mut registry = ToolRegistry::new();
        registry.register(ToolSpec::new(
            name,
            "test tool",
            empty_parameters(),
            |_args| async move { Ok(String::new()) },
        ));
        registry
    }

    /// 构造测试用工具调用。
    ///
    /// 参数:
    /// - `name`: 工具名
    /// - `arguments`: 原始参数文本
    ///
    /// 返回:
    /// - 工具调用
    fn call(name: &str, arguments: &str) -> ToolCall {
        ToolCall {
            id: "call-1".to_string(),
            kind: "function".to_string(),
            function: ToolCallFunction {
                name: name.to_string(),
                arguments: arguments.to_string(),
            },
        }
    }

    /// 验证未知工具名被拒绝而不是终止整轮。
    #[test]
    fn unknown_tool_is_rejected_with_available_names() {
        let registry = registry_with("read_file");
        let visibility = super::super::tool_visibility::ToolVisibility::new(Vec::new());

        let gate = evaluate_tool_gate(
            &registry,
            &visibility,
            &call("hallucinated_tool", "{}"),
            &[],
        );

        let ToolGate::Reject(message) = gate else {
            panic!("unknown tool must be rejected");
        };
        assert!(message.starts_with("tool error: unknown tool: hallucinated_tool"));
        assert!(message.contains("read_file"));
    }

    /// 验证畸形 JSON 参数被拒绝而不是终止整轮。
    #[test]
    fn malformed_arguments_are_rejected() {
        let registry = registry_with("read_file");
        let visibility = super::super::tool_visibility::ToolVisibility::new(Vec::new());

        let gate = evaluate_tool_gate(
            &registry,
            &visibility,
            &call("read_file", "{\"path\": \"/tmp/a"),
            &[],
        );

        let ToolGate::Reject(message) = gate else {
            panic!("malformed arguments must be rejected");
        };
        assert!(message.contains("invalid arguments for read_file"));
    }

    /// 验证参数错误包含具体非法字段，供模型修正调用。
    #[test]
    fn schema_error_identifies_the_unexpected_argument() {
        let mut registry = ToolRegistry::new();
        registry.register(ToolSpec::new(
            "read_file",
            "test tool",
            serde_json::json!({
                "type": "object",
                "properties": {"path": {"type": "string"}},
                "additionalProperties": false
            }),
            |_args| async move { Ok(String::new()) },
        ));
        let visibility = super::super::tool_visibility::ToolVisibility::new(Vec::new());

        let gate = evaluate_tool_gate(
            &registry,
            &visibility,
            &call("read_file", r#"{"action":"read","path":"/tmp/a"}"#),
            &[],
        );

        let ToolGate::Reject(message) = gate else {
            panic!("unexpected argument must be rejected");
        };
        assert!(message.contains("action"), "{message}");
    }

    /// 验证非对象参数被拒绝。
    #[test]
    fn non_object_arguments_are_rejected() {
        let registry = registry_with("read_file");
        let visibility = super::super::tool_visibility::ToolVisibility::new(Vec::new());

        let gate = evaluate_tool_gate(&registry, &visibility, &call("read_file", "[1, 2]"), &[]);

        assert!(matches!(gate, ToolGate::Reject(_)));
    }

    /// 验证空参数按空对象放行。
    #[test]
    fn empty_arguments_are_accepted() {
        let registry = registry_with("read_file");
        let visibility = super::super::tool_visibility::ToolVisibility::new(Vec::new());

        let gate = evaluate_tool_gate(&registry, &visibility, &call("read_file", ""), &[]);

        assert_eq!(gate, ToolGate::Proceed);
    }

    /// 验证渐进式加载下未加载的工具提示先调用 load。
    #[test]
    fn deferred_tool_asks_for_load_first() {
        let registry = registry_with("web_search");
        let visibility =
            super::super::tool_visibility::ToolVisibility::new(vec!["web_search".to_string()]);

        let gate = evaluate_tool_gate(&registry, &visibility, &call("web_search", "{}"), &[]);

        let ToolGate::Reject(message) = gate else {
            panic!("deferred tool must be rejected before load");
        };
        assert!(message.contains("is not loaded in the current visible tool set"));
    }

    /// 验证通配延迟配置不会隐藏基础问题工具。
    #[test]
    fn wildcard_progressive_mode_keeps_base_question_tool_visible() {
        let registry = registry_with("ask_question");
        let visibility = super::super::tool_visibility::ToolVisibility::new(vec!["*".to_string()]);

        let gate = evaluate_tool_gate(&registry, &visibility, &call("ask_question", "{}"), &[]);

        assert_eq!(gate, ToolGate::Proceed);
    }

    /// 验证 AUR 安装与审查同轮调用被拒绝。
    #[test]
    fn aur_install_after_review_in_same_turn_is_rejected() {
        let registry = registry_with("install_aur_package");
        let visibility = super::super::tool_visibility::ToolVisibility::new(Vec::new());

        let gate = evaluate_tool_gate(
            &registry,
            &visibility,
            &call("install_aur_package", "{}"),
            &["review_aur_package".to_string()],
        );

        let ToolGate::Reject(message) = gate else {
            panic!("same-turn install must be rejected");
        };
        assert!(message.contains("cannot run in the same turn"));
    }
}
