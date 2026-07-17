use super::AgentMode;
use crate::llm::{ChatContent, ChatContentPart, ChatMessage};
use chrono::Local;
use std::io::IsTerminal;

/// 追加稳定模式提醒。
///
/// 参数:
/// - `system_prompt`: 基础系统提示
/// - `mode`: Agent 模式
///
/// 返回:
/// - 稳定系统提示
#[allow(dead_code)]
pub(super) fn with_mode_reminder(system_prompt: String, mode: AgentMode) -> String {
    format!("{system_prompt}\n\n{}", mode.reminder())
}

/// 将系统消息合并为首条消息。
///
/// 参数:
/// - `messages`: 待发送给模型的消息列表
///
/// 返回:
/// - 单条系统消息前置后的消息列表
pub(super) fn system_messages_first(messages: Vec<ChatMessage>) -> Vec<ChatMessage> {
    let mut system_parts = Vec::new();
    let mut other_messages = Vec::new();
    for message in messages {
        if message.role == "system" {
            let text = chat_content_text(message.content);
            if !text.trim().is_empty() {
                system_parts.push(text);
            }
        } else {
            other_messages.push(message);
        }
    }
    if system_parts.is_empty() {
        return other_messages;
    }
    let mut ordered = Vec::with_capacity(other_messages.len() + 1);
    ordered.push(ChatMessage::system(system_parts.join("\n\n")));
    ordered.extend(other_messages);
    ordered
}

/// 构造动态运行时上下文消息。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 当前轮运行时上下文
pub(super) fn runtime_context_message() -> String {
    let cwd = crate::runtime_cwd::current_dir()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let runtime = terminal_runtime_context();
    format!(
        "<system-reminder>\n当前系统时间：{}。用户询问当前时间时，优先使用这里的时间，不需要调用命令查询。\n当前工作目录：{cwd}。涉及相对路径、当前项目、文件操作时优先以此为准。\n{runtime}\n</system-reminder>",
        Local::now().format("%Y年%m月%d日 %A %H:%M")
    )
}

/// 清理用户可见输入中的运行时提醒。
///
/// 参数:
/// - `input`: 原始用户输入
///
/// 返回:
/// - 清理后的用户输入
pub(super) fn clean_user_visible_text(input: &str) -> String {
    let mut output = input.to_string();
    for tag in ["system-reminder", "system_reminder"] {
        output = strip_tagged_sections(output, tag);
    }
    output
}

/// 提取消息文本内容。
///
/// 参数:
/// - `content`: 聊天消息内容
///
/// 返回:
/// - 文本内容，图片部分会被忽略
fn chat_content_text(content: Option<ChatContent>) -> String {
    match content {
        Some(ChatContent::Text(text)) => text,
        Some(ChatContent::Parts(parts)) => parts
            .into_iter()
            .filter_map(|part| match part {
                ChatContentPart::Text { text } => Some(text),
                ChatContentPart::ImageUrl { .. } => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        None => String::new(),
    }
}

/// 构造终端运行环境上下文。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 当前终端环境描述
fn terminal_runtime_context() -> String {
    let stdin_tty = std::io::stdin().is_terminal();
    let stdout_tty = std::io::stdout().is_terminal();
    let stderr_tty = std::io::stderr().is_terminal();
    let environment = if stdin_tty || stdout_tty || stderr_tty {
        if crate::i18n::is_zh() {
            "终端会话"
        } else {
            "terminal session"
        }
    } else if crate::i18n::is_zh() {
        "非交互或管道环境"
    } else {
        "non-interactive or piped environment"
    };
    let shell = std::env::var("SHELL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    let mut terminal_parts = Vec::new();
    for key in ["TERM_PROGRAM", "TERM", "COLORTERM"] {
        if let Ok(value) = std::env::var(key) {
            if !value.trim().is_empty() {
                terminal_parts.push(format!("{key}={value}"));
            }
        }
    }
    let terminal = if terminal_parts.is_empty() {
        "unknown".to_string()
    } else {
        terminal_parts.join(", ")
    };
    if crate::i18n::is_zh() {
        format!("当前运行环境：{environment}。当前 shell：{shell}。当前终端标识：{terminal}。")
    } else {
        format!("Current runtime environment: {environment}. Current shell: {shell}. Terminal identifiers: {terminal}.")
    }
}

/// 删除指定 XML 标签包裹的内容。
///
/// 参数:
/// - `text`: 原始文本
/// - `tag`: 标签名称
///
/// 返回:
/// - 删除标签片段后的文本
fn strip_tagged_sections(mut text: String, tag: &str) -> String {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    while let Some(start) = text.find(&open) {
        let Some(relative_end) = text[start..].find(&close) else {
            text.replace_range(start.., "");
            break;
        };
        let end = start + relative_end + close.len();
        text.replace_range(start..end, "");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_pasted_system_reminder_from_user_input() {
        let input = "继续<system-reminder>hidden</system-reminder> ok";

        assert_eq!(clean_user_visible_text(input), "继续 ok");
    }

    #[test]
    fn strips_unclosed_system_reminder_from_user_input() {
        let input = "继续<system_reminder>hidden";

        assert_eq!(clean_user_visible_text(input), "继续");
    }

    #[test]
    fn stable_prompt_does_not_include_runtime_context() {
        let prompt = with_mode_reminder("base".to_string(), AgentMode::Yolo);

        assert!(prompt.contains("base"));
        assert!(!prompt.contains("当前系统时间"));
        assert!(runtime_context_message().contains("当前系统时间"));
    }

    #[test]
    fn system_messages_are_merged_before_history() {
        let messages = vec![
            ChatMessage::system("base"),
            ChatMessage::plain("user", "old input"),
            ChatMessage::plain("assistant", "old reply"),
            ChatMessage::system("runtime"),
            ChatMessage::plain("user", "new input"),
        ];

        let ordered = system_messages_first(messages);
        let roles = ordered
            .iter()
            .map(|message| message.role.as_str())
            .collect::<Vec<_>>();
        let system_text = ordered
            .first()
            .and_then(|message| message.content.clone())
            .map(|content| chat_content_text(Some(content)))
            .unwrap();

        assert_eq!(roles, ["system", "user", "assistant", "user"]);
        assert!(system_text.contains("base"));
        assert!(system_text.contains("runtime"));
        assert_eq!(
            ordered
                .into_iter()
                .filter(|message| message.role == "system")
                .count(),
            1
        );
    }

    #[test]
    fn system_normalization_keeps_messages_without_system() {
        let messages = vec![
            ChatMessage::plain("user", "old input"),
            ChatMessage::plain("assistant", "old reply"),
        ];

        let roles = system_messages_first(messages)
            .into_iter()
            .map(|message| message.role)
            .collect::<Vec<_>>();

        assert_eq!(roles, ["user", "assistant"]);
    }
}
