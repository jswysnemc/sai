use super::repl_commands::{
    visible_repl_command_suggestions, ReplCommandSuggestion, StreamCommandPolicy,
};
use crate::i18n::text as t;

#[cfg(test)]
mod tests;

/// 【终端】【参数补全】提供已有命令支持的固定参数，input 为输入，streaming 为生成状态
/// 返回: 完整命令候选，保持面板和执行端使用相同文本
pub(super) fn arguments(input: &str, streaming: bool) -> Vec<ReplCommandSuggestion> {
    let Some((command, rest)) = input.split_once(char::is_whitespace) else {
        return Vec::new();
    };
    let values: &[(&str, &str)] = match command.to_ascii_lowercase().as_str() {
        "/context" => &[
            ("/context edit", t("edit compaction policy", "编辑压缩策略")),
            (
                "/context reset",
                t("restore global defaults", "恢复全局默认策略"),
            ),
        ],
        "/goal" => &[
            ("/goal pause", t("pause the current goal", "暂停当前目标")),
            ("/goal resume", t("resume the current goal", "继续当前目标")),
            ("/goal clear", t("clear the current goal", "清除当前目标")),
        ],
        "/clear" => &[(
            "/clear all",
            t("clear all conversation data", "清空全部对话数据"),
        )],
        _ => &[],
    };
    let prefix = format!(
        "{} {}",
        command.to_ascii_lowercase(),
        rest.trim_start().to_ascii_lowercase()
    );
    values
        .iter()
        .filter(|(value, _)| value.starts_with(&prefix))
        .map(|&(command, description)| ReplCommandSuggestion {
            command,
            description,
            disabled: streaming
                && super::repl_commands::stream_command_policy(command)
                    == StreamCommandPolicy::Disabled,
        })
        .collect()
}

/// 接受当前候选；input/cursor 为草稿，selected 为候选序号，enter 为回车标记
/// streaming 为生成状态；返回替换文本，完整命令的回车返回空以继续提交
pub(super) fn accept(
    input: &str,
    cursor: usize,
    selected: usize,
    streaming: bool,
    enter: bool,
) -> Option<String> {
    if cursor != input.chars().count() {
        return None;
    }
    let suggestions = visible_repl_command_suggestions(input, streaming);
    let item = suggestions.get(selected.min(suggestions.len().saturating_sub(1)))?;
    if item.disabled || (enter && input.trim().eq_ignore_ascii_case(item.command)) {
        return None;
    }
    let mut replacement = item.command.to_string();
    if matches!(item.command, "/context" | "/goal" | "/clear") {
        replacement.push(' ');
    }
    Some(replacement)
}
