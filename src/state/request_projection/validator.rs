use super::model::{ProjectedRequest, ProjectionWarning};
use crate::llm::{ChatContent, ChatMessage};
use std::collections::{HashMap, HashSet};

/// 校验 provider 请求投影视图。
///
/// 参数:
/// - `projection`: 待校验的 provider 请求投影视图
///
/// 返回:
/// - 阶段 0 只返回警告，不阻断请求
pub(crate) fn validate_provider_projection(
    projection: &ProjectedRequest,
) -> Vec<ProjectionWarning> {
    let mut warnings = Vec::new();
    if projection.messages.is_empty() {
        warnings.push(ProjectionWarning {
            message: "provider projection has no messages".to_string(),
        });
    }
    if projection.estimate.context_limit_chars == 0 {
        warnings.push(ProjectionWarning {
            message: "provider projection has invalid context limit".to_string(),
        });
    }
    let runtime_reminders = projection
        .messages
        .iter()
        .filter(|message| message.role == "system")
        .filter(|message| contains_runtime_reminder(message.content.as_ref()))
        .count();
    if runtime_reminders > 1 {
        warnings.push(ProjectionWarning {
            message: "provider projection has duplicate runtime reminders".to_string(),
        });
    }
    warnings.extend(validate_tool_pairing(&projection.messages));
    warnings
}

/// 返回第一个需要阻断 provider request 的工具配对警告。
///
/// 参数:
/// - `projection`: 待校验的 provider 请求投影视图
///
/// 返回:
/// - 第一个阻断级工具配对警告
pub(crate) fn first_blocking_tool_pairing_warning(
    projection: &ProjectedRequest,
) -> Option<ProjectionWarning> {
    projection
        .warnings
        .iter()
        .find(|warning| is_blocking_tool_pairing_warning(&warning.message))
        .cloned()
}

/// 判断 warning 是否属于 provider API 级工具配对错误。
///
/// 参数:
/// - `message`: warning 文本
///
/// 返回:
/// - 是否需要阻断请求
fn is_blocking_tool_pairing_warning(message: &str) -> bool {
    message.contains("tool call without result")
        || message.contains("orphan tool result")
        || message.contains("duplicate tool result")
        || message.contains("duplicate tool call id")
        || message.contains("tool result without call id")
}

/// 校验 provider 请求中的工具调用配对。
///
/// 参数:
/// - `messages`: provider 请求消息列表
///
/// 返回:
/// - 工具配对警告列表
fn validate_tool_pairing(messages: &[ChatMessage]) -> Vec<ProjectionWarning> {
    let mut warnings = Vec::new();
    let mut calls = HashMap::new();
    let mut duplicate_calls = HashSet::new();
    let mut results = HashSet::new();
    for message in messages {
        if let Some(tool_calls) = &message.tool_calls {
            for call in tool_calls {
                if calls
                    .insert(call.id.clone(), call.function.name.clone())
                    .is_some()
                {
                    duplicate_calls.insert(call.id.clone());
                }
            }
        }
        if message.role == "tool" {
            if let Some(call_id) = &message.tool_call_id {
                if !results.insert(call_id.clone()) {
                    warnings.push(ProjectionWarning {
                        message: format!(
                            "provider projection has duplicate tool result: {call_id}"
                        ),
                    });
                }
            } else {
                warnings.push(ProjectionWarning {
                    message: "provider projection has tool result without call id".to_string(),
                });
            }
        }
    }
    for call_id in duplicate_calls {
        warnings.push(ProjectionWarning {
            message: format!("provider projection has duplicate tool call id: {call_id}"),
        });
    }
    for call_id in calls.keys() {
        if !results.contains(call_id) {
            warnings.push(ProjectionWarning {
                message: format!("provider projection has tool call without result: {call_id}"),
            });
        }
    }
    for call_id in results {
        if !calls.contains_key(&call_id) {
            warnings.push(ProjectionWarning {
                message: format!("provider projection has orphan tool result: {call_id}"),
            });
        }
    }
    warnings
}

/// 判断消息内容是否包含运行时提醒标签。
///
/// 参数:
/// - `content`: 聊天消息内容
///
/// 返回:
/// - 是否包含运行时提醒
fn contains_runtime_reminder(content: Option<&ChatContent>) -> bool {
    let text = match content {
        Some(ChatContent::Text(text)) => text.as_str(),
        Some(ChatContent::Parts(_)) | None => return false,
    };
    text.contains("<system-reminder") || text.contains("<system_reminder")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::ChatMessage;
    use crate::state::request_projection::model::{
        ProjectedRequest, ProjectionEstimate, ProjectionKind,
    };

    fn projection(messages: Vec<ChatMessage>, context_limit_chars: usize) -> ProjectedRequest {
        ProjectedRequest {
            kind: ProjectionKind::ProviderTurn,
            messages,
            tool_count: 0,
            estimate: ProjectionEstimate {
                message_chars: 0,
                state_context_chars: 0,
                context_limit_chars,
                context_ratio: 0.0,
            },
            dynamic_sources: Vec::new(),
            warnings: Vec::new(),
        }
    }

    #[test]
    fn warns_about_invalid_context_limit_and_duplicate_runtime_context() {
        let projection = projection(
            vec![
                ChatMessage::system("<system-reminder>first</system-reminder>"),
                ChatMessage::plain("user", "hello"),
                ChatMessage::system("<system-reminder>second</system-reminder>"),
            ],
            0,
        );

        let warnings = validate_provider_projection(&projection)
            .into_iter()
            .map(|warning| warning.message)
            .collect::<Vec<_>>();

        assert!(warnings
            .iter()
            .any(|message| message == "provider projection has invalid context limit"));
        assert!(warnings
            .iter()
            .any(|message| message == "provider projection has duplicate runtime reminders"));
    }

    #[test]
    fn warns_about_invalid_tool_pairing() {
        let mut assistant = ChatMessage::assistant(
            "",
            Some(vec![crate::llm::ToolCall {
                id: "call_1".to_string(),
                kind: "function".to_string(),
                function: crate::llm::ToolCallFunction {
                    name: "read_file".to_string(),
                    arguments: "{}".to_string(),
                },
            }]),
        );
        assistant
            .tool_calls
            .as_mut()
            .unwrap()
            .push(crate::llm::ToolCall {
                id: "call_1".to_string(),
                kind: "function".to_string(),
                function: crate::llm::ToolCallFunction {
                    name: "read_file".to_string(),
                    arguments: "{}".to_string(),
                },
            });
        let projection = projection(
            vec![assistant, ChatMessage::tool("call_2", "orphan result")],
            1_000,
        );

        let warnings = validate_provider_projection(&projection)
            .into_iter()
            .map(|warning| warning.message)
            .collect::<Vec<_>>();

        assert!(warnings
            .iter()
            .any(|message| message.contains("duplicate tool call id: call_1")));
        assert!(warnings
            .iter()
            .any(|message| message.contains("tool call without result: call_1")));
        assert!(warnings
            .iter()
            .any(|message| message.contains("orphan tool result: call_2")));
    }

    #[test]
    fn returns_first_blocking_tool_pairing_warning() {
        let assistant = ChatMessage::assistant(
            "",
            Some(vec![crate::llm::ToolCall {
                id: "call_1".to_string(),
                kind: "function".to_string(),
                function: crate::llm::ToolCallFunction {
                    name: "read_file".to_string(),
                    arguments: "{}".to_string(),
                },
            }]),
        );
        let mut projection = projection(vec![assistant], 1_000);
        projection.warnings = validate_provider_projection(&projection);

        let warning = first_blocking_tool_pairing_warning(&projection).unwrap();

        assert!(warning.message.contains("tool call without result"));
    }
}
