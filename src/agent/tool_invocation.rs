use super::repeat_guard::RepeatGuard;
use super::tool_gate::tool_error_output;
use super::tool_visibility::ToolVisibility;
use super::{Agent, AgentEvent};
use crate::llm::{ChatMessage, ToolCall, ToolCallFunction};
use crate::tools::{self, ToolPermission, ToolRegistry};
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::Value;

pub(super) const MAX_QUESTION_ROUNDS_PER_TURN: usize = 8;

/// 构造问题工具超过单轮限制时的错误文本。
///
/// 返回:
/// - 包含当前限制值的工具错误文本
pub(super) fn question_limit_notice() -> String {
    format!(
        "tool error: ask_question exceeded the per-turn limit of {MAX_QUESTION_ROUNDS_PER_TURN}"
    )
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InvokeRequest {
    tool_name: String,
    arguments: Value,
}

/// 单轮供应商调用的统一解析结果。
pub(super) struct PreparedToolCalls {
    /// 供应商原始调用及其真实执行调用
    pub(super) calls: Vec<(ToolCall, Result<ToolCall>)>,
    /// 已加载且参数合法的问题工具调用数量
    pub(super) question_call_count: usize,
}

/// 已预分配事件序号的单个工具调用。
pub(super) struct SequencedToolCall {
    pub(super) sequence: usize,
    pub(super) provider_call: ToolCall,
    pub(super) execution_call: Result<ToolCall>,
}

/// 同一模型子轮中的工具执行组。
pub(super) enum ToolExecutionGroup {
    /// 单个调用保持完整串行流程
    Serial(SequencedToolCall),
    /// 连续且无需交互授权的只读调用可以并发执行
    ConcurrentReadOnly(Vec<SequencedToolCall>),
}

impl PreparedToolCalls {
    /// 计算本批问题调用的轮次限制与兄弟工具延迟策略。
    ///
    /// 参数:
    /// - `question_rounds`: 当前用户轮次已经进入的问题轮数
    ///
    /// 返回:
    /// - 问题调用数量、是否允许进入问题流程、是否延迟兄弟工具
    pub(super) fn question_policy(&self, question_rounds: &mut usize) -> (usize, bool, bool) {
        if self.question_call_count == 1 {
            *question_rounds += 1;
        }
        (
            self.question_call_count,
            self.question_call_count == 1 && *question_rounds <= MAX_QUESTION_ROUNDS_PER_TURN,
            self.question_call_count == 1 && self.calls.len() > 1,
        )
    }

    /// 按执行副作用把调用分成串行项与连续只读组。
    ///
    /// 参数:
    /// - `registry`: 当前会话工具注册表
    /// - `visibility`: 渐进式工具可见性
    /// - `first_sequence`: 本批首个工具事件序号
    ///
    /// 返回:
    /// - 保持供应商调用顺序的执行组
    pub(super) fn into_execution_groups(
        self,
        registry: &ToolRegistry,
        visibility: &ToolVisibility,
        first_sequence: usize,
    ) -> Vec<ToolExecutionGroup> {
        let mut groups = Vec::new();
        let mut read_only = Vec::new();

        for (index, (provider_call, execution_call)) in self.calls.into_iter().enumerate() {
            let concurrent = execution_call
                .as_ref()
                .is_ok_and(|call| is_concurrent_read_only_call(registry, visibility, call));
            let call = SequencedToolCall {
                sequence: first_sequence.saturating_add(index),
                provider_call,
                execution_call,
            };
            if concurrent {
                read_only.push(call);
                continue;
            }
            flush_read_only_group(&mut groups, &mut read_only);
            groups.push(ToolExecutionGroup::Serial(call));
        }
        flush_read_only_group(&mut groups, &mut read_only);
        groups
    }
}

/// 判断调用能否进入无需交互的只读并发组。
fn is_concurrent_read_only_call(
    registry: &ToolRegistry,
    visibility: &ToolVisibility,
    call: &ToolCall,
) -> bool {
    if call.function.name == "ask_question" || visibility.is_loader_call(&call.function.name) {
        return false;
    }
    registry
        .permission(&call.function.name)
        .is_ok_and(|permission| {
            permission == ToolPermission::ReadOnly
                && registry
                    .requires_permission(&call.function.name, &call.function.arguments)
                    .is_ok_and(|required| !required)
        })
}

/// 把累积的连续只读调用写入执行组。
fn flush_read_only_group(
    groups: &mut Vec<ToolExecutionGroup>,
    read_only: &mut Vec<SequencedToolCall>,
) {
    match read_only.len() {
        0 => {}
        1 => groups.push(ToolExecutionGroup::Serial(read_only.pop().unwrap())),
        _ => groups.push(ToolExecutionGroup::ConcurrentReadOnly(std::mem::take(
            read_only,
        ))),
    }
}

/// 解析一轮供应商工具调用并统计可进入专用流程的问题调用。
///
/// 参数:
/// - `visibility`: 当前 Agent 的渐进式工具状态
/// - `registry`: 当前会话工具注册表
/// - `provider_calls`: 供应商返回的原始工具调用
///
/// 返回:
/// - 保留原始调用的解析结果及有效问题调用数量
pub(super) fn prepare_tool_calls(
    visibility: &ToolVisibility,
    registry: &ToolRegistry,
    provider_calls: &[ToolCall],
) -> PreparedToolCalls {
    // 1. 每个供应商调用独立解包，畸形外壳留给循环按工具错误回传
    let calls = provider_calls
        .iter()
        .cloned()
        .map(|provider_call| {
            let execution_call = resolve_execution_call(visibility, &provider_call);
            (provider_call, execution_call)
        })
        .collect::<Vec<_>>();
    // 2. 未加载或参数不合法的问题调用不能影响问题轮次与兄弟工具调度
    let question_call_count = calls
        .iter()
        .filter(|(_, call)| {
            registry.contains("ask_question")
                && call.as_ref().is_ok_and(|call| {
                    call.function.name == "ask_question"
                        && visibility.is_visible("ask_question")
                        && registry
                            .validate_arguments("ask_question", &call.function.arguments)
                            .is_ok()
                })
        })
        .count();
    PreparedToolCalls {
        calls,
        question_call_count,
    }
}

impl Agent {
    /// 记录供应商工具调用，并把无法解包的调用作为可恢复错误写回历史。
    ///
    /// 参数:
    /// - `turn_id`: 当前轮次标识
    /// - `sequence`: 当前轮内工具调用顺序
    /// - `assistant_round`: 产生调用的模型子轮编号
    /// - `assistant_reasoning`: 模型子轮思考内容
    /// - `provider_call`: 供应商返回的原始调用
    /// - `execution_call`: 统一外壳的解析结果
    /// - `messages`: 当前模型消息列表
    /// - `repeat_guard`: 重复调用防护状态
    /// - `on_event`: Agent 事件回调
    ///
    /// 返回:
    /// - 解包成功时返回真实调用；失败时完成错误记录并返回 None
    #[allow(clippy::too_many_arguments)]
    pub(super) fn begin_tool_invocation<F>(
        &self,
        turn_id: &str,
        sequence: usize,
        assistant_round: usize,
        assistant_reasoning: Option<&str>,
        provider_call: &ToolCall,
        execution_call: Result<ToolCall>,
        messages: &mut Vec<ChatMessage>,
        repeat_guard: &mut RepeatGuard,
        on_event: &mut F,
    ) -> Result<Option<ToolCall>>
    where
        F: FnMut(AgentEvent) -> Result<()>,
    {
        self.record_tool_call_started(
            turn_id,
            sequence,
            assistant_round,
            assistant_reasoning,
            provider_call,
        )?;
        let call = match execution_call {
            Ok(call) => call,
            Err(error) => {
                let verdict = repeat_guard.observe(
                    &provider_call.function.name,
                    &provider_call.function.arguments,
                );
                let output = match verdict {
                    super::repeat_guard::RepeatVerdict::Stop { seen } => {
                        super::repeat_guard::stop_notice(&provider_call.function.name, seen)
                    }
                    _ => tool_error_output(&error),
                };
                repeat_guard.observe_rejected(
                    &provider_call.function.name,
                    &provider_call.function.arguments,
                );
                self.record_simple_tool_result(turn_id, provider_call, false, &output)?;
                on_event(AgentEvent::ToolResult {
                    name: provider_call.function.name.clone(),
                    ok: false,
                    output: output.clone(),
                })?;
                messages.push(ChatMessage::tool(provider_call.id.clone(), output));
                return Ok(None);
            }
        };
        self.record_tool_call_display(&provider_call.id, &call)?;
        Ok(Some(call))
    }
}

/// 将供应商看到的统一调用外壳还原为真实工具调用。
///
/// 参数:
/// - `visibility`: 当前 Agent 的渐进式工具状态
/// - `provider_call`: 供应商返回的原始工具调用
///
/// 返回:
/// - 非渐进模式保持原调用；渐进模式返回解包后的真实调用
pub(crate) fn resolve_execution_call(
    visibility: &ToolVisibility,
    provider_call: &ToolCall,
) -> Result<ToolCall> {
    if !visibility.is_progressive() || visibility.is_loader_call(&provider_call.function.name) {
        return Ok(provider_call.clone());
    }
    if !visibility.is_invoker_call(&provider_call.function.name) {
        if !visibility.requires_load(&provider_call.function.name) {
            return Ok(provider_call.clone());
        }
        bail!(
            "direct tool calls are disabled in progressive mode; call {} after loading the target schema",
            tools::INVOKE_NAME
        );
    }

    // 1. 严格解析统一外壳，避免额外字段或非对象参数绕过真实工具校验
    let request = serde_json::from_str::<InvokeRequest>(&provider_call.function.arguments)
        .context("invalid invoke_tool arguments")?;
    let tool_name = request.tool_name.trim();
    if tool_name.is_empty() {
        bail!("invoke_tool tool_name must be non-empty");
    }
    if tool_name == tools::LOAD_NAME || tool_name == tools::INVOKE_NAME {
        bail!("invoke_tool cannot target gateway tool: {tool_name}");
    }
    if !request.arguments.is_object() {
        bail!("invoke_tool arguments must be a JSON object");
    }

    // 2. 保留 provider call_id，真实名称和参数交给原有治理与执行链路
    Ok(ToolCall {
        id: provider_call.id.clone(),
        kind: provider_call.kind.clone(),
        function: ToolCallFunction {
            name: tool_name.to_string(),
            arguments: serde_json::to_string(&request.arguments)?,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证统一外壳解包后保留调用标识并输出真实参数。
    #[test]
    fn resolves_invoker_to_real_tool_call() {
        let visibility = ToolVisibility::new(vec!["*".to_string()]);
        let provider_call = call(
            tools::INVOKE_NAME,
            r#"{"tool_name":"read_file","arguments":{"path":"README.md"}}"#,
        );

        let resolved = resolve_execution_call(&visibility, &provider_call).unwrap();

        assert_eq!(resolved.id, "call_1");
        assert_eq!(resolved.function.name, "read_file");
        assert_eq!(
            serde_json::from_str::<Value>(&resolved.function.arguments).unwrap(),
            serde_json::json!({"path": "README.md"})
        );
    }

    /// 验证渐进模式允许直接调用具备原生定义的基础工具。
    #[test]
    fn accepts_direct_base_tool_call_in_progressive_mode() {
        let visibility = ToolVisibility::new(vec!["*".to_string()]);
        let provider_call = call("read_file", r#"{"path":"a"}"#);

        let resolved = resolve_execution_call(&visibility, &provider_call).unwrap();

        assert_eq!(resolved.id, provider_call.id);
        assert_eq!(resolved.function.name, provider_call.function.name);
        assert_eq!(
            resolved.function.arguments,
            provider_call.function.arguments
        );
    }

    /// 验证渐进模式仍拒绝直接调用需要加载的工具。
    #[test]
    fn rejects_direct_deferred_tool_call_in_progressive_mode() {
        let visibility = ToolVisibility::new(vec!["*".to_string()]);

        let error = resolve_execution_call(&visibility, &call("web_search", r#"{"query":"rust"}"#))
            .unwrap_err();

        assert!(error.to_string().contains("direct tool calls are disabled"));
    }

    /// 验证基础问题工具直接调用时正常进入问题流程。
    #[test]
    fn direct_base_question_activates_question_policy() {
        let visibility = ToolVisibility::new(vec!["*".to_string()]);
        let mut registry = ToolRegistry::new();
        registry.register(crate::tools::ToolSpec::new(
            "ask_question",
            "test",
            crate::tools::empty_parameters(),
            |_arguments| async move { Ok(String::new()) },
        ));
        let calls = vec![call("ask_question", "{}"), call("read_file", "{}")];

        let prepared = prepare_tool_calls(&visibility, &registry, &calls);
        let mut rounds = 0;

        assert_eq!(prepared.question_policy(&mut rounds), (1, true, true));
        assert_eq!(rounds, 1);
    }

    /// 验证连续只读调用并发分组，写调用会切断分组且序号预先稳定分配。
    #[test]
    fn groups_contiguous_read_only_calls_and_preserves_sequences() {
        let visibility = ToolVisibility::new(Vec::new());
        let mut registry = ToolRegistry::new();
        registry.register(crate::tools::ToolSpec::new(
            "read_a",
            "read",
            tools::empty_parameters(),
            |_| async { Ok(String::new()) },
        ));
        registry.register(crate::tools::ToolSpec::new(
            "read_b",
            "read",
            tools::empty_parameters(),
            |_| async { Ok(String::new()) },
        ));
        registry.register(
            crate::tools::ToolSpec::new("write", "write", tools::empty_parameters(), |_| async {
                Ok(String::new())
            })
            .writes(),
        );
        let calls = vec![
            call("read_a", "{}"),
            call("read_b", "{}"),
            call("write", "{}"),
            call("read_a", "{}"),
        ];

        let groups = prepare_tool_calls(&visibility, &registry, &calls).into_execution_groups(
            &registry,
            &visibility,
            11,
        );

        assert!(
            matches!(groups[0], ToolExecutionGroup::ConcurrentReadOnly(ref group) if group.len() == 2)
        );
        assert!(matches!(groups[1], ToolExecutionGroup::Serial(ref call) if call.sequence == 13));
        assert!(matches!(groups[2], ToolExecutionGroup::Serial(ref call) if call.sequence == 14));
    }

    /// 构造测试用供应商工具调用。
    ///
    /// 参数:
    /// - `name`: 工具名称
    /// - `arguments`: JSON 参数
    ///
    /// 返回:
    /// - 固定调用标识的工具调用
    fn call(name: &str, arguments: &str) -> ToolCall {
        ToolCall {
            id: "call_1".to_string(),
            kind: "function".to_string(),
            function: ToolCallFunction {
                name: name.to_string(),
                arguments: arguments.to_string(),
            },
        }
    }
}
