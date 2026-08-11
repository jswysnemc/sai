/// 返回 REPL 支持的斜杠菜单。
///
/// 返回:
/// - 斜杠菜单列表
pub(super) fn repl_commands() -> &'static [&'static str] {
    crate::control_commands::catalog::REPL_COMMANDS
}

/// 判断输入是否为退出 REPL 的命令。
///
/// 参数:
/// - `input`: 已输入或提交的原始文本
///
/// 返回:
/// - 匹配 `exit`、`quit` 或 `/exit` 时返回 true
pub(super) fn is_repl_exit_command(input: &str) -> bool {
    let input = input.trim();
    input.eq_ignore_ascii_case("exit")
        || input.eq_ignore_ascii_case("quit")
        || input.eq_ignore_ascii_case("/exit")
}

/// 根据当前输入生成斜杠菜单补全建议。
///
/// 参数:
/// - `input`: 当前输入内容
///
/// 返回:
/// - 可补全的斜杠菜单
pub(super) fn repl_command_suggestions(input: &str) -> Vec<ReplCommandSuggestion> {
    if !input.starts_with('/') {
        return Vec::new();
    }
    // 前缀匹配大小写不敏感，与执行侧的 eq_ignore_ascii_case 口径一致
    let lowered = input.to_ascii_lowercase();
    repl_commands()
        .iter()
        .copied()
        .filter(|command| command.starts_with(&lowered))
        .map(|command| ReplCommandSuggestion {
            command,
            description: command_description(command),
        })
        .collect()
}

/// 为命令形态的未知输入生成提示。
///
/// 参数:
/// - `input`: 已去除首尾空白的输入
/// - `external_engine`: 当前是否由外部 ACP 内核执行对话
///
/// 返回:
/// - 命令形态但未被任何分发命中时返回提示文本；路径等普通文本返回空
pub(super) fn unknown_slash_command_hint(input: &str, external_engine: bool) -> Option<String> {
    // 外部内核有自己的斜杠命令（Claude Code 的 /review 等），
    // sai 无从知道对方支持哪些；拦下来会让这些命令彻底发不出去，
    // 因此一律放行交给内核判断——它不认识时会自己回复说明
    if external_engine {
        return None;
    }
    let candidate = input.strip_prefix('/')?;
    let token = candidate.split_whitespace().next().unwrap_or_default();
    // 只拦截命令形态的词：/home/user 这类路径按普通消息放行
    if token.is_empty()
        || !token
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return None;
    }
    Some(if crate::i18n::is_zh() {
        format!("未识别的命令用法：/{token}；输入 /help 查看可用命令")
    } else {
        format!("unrecognized command usage: /{token}; run /help to list commands")
    })
}

/// 根据当前输入生成面板可见的斜杠菜单建议。
///
/// 参数:
/// - `input`: 当前输入内容
///
/// 返回:
/// - 不超过面板容量的补全建议
pub(super) fn visible_repl_command_suggestions(input: &str) -> Vec<ReplCommandSuggestion> {
    repl_command_suggestions(input)
        .into_iter()
        .take(MAX_REPL_COMMAND_SUGGESTIONS)
        .collect()
}

/// 返回唯一匹配的斜杠菜单补全文本。
///
/// 参数:
/// - `input`: 当前输入内容
///
/// 返回:
/// - 唯一补全结果
pub(super) fn complete_repl_command(input: &str) -> Option<&'static str> {
    let suggestions = repl_command_suggestions(input);
    if suggestions.len() == 1 {
        suggestions.first().map(|suggestion| suggestion.command)
    } else {
        None
    }
}

/// 返回 slash 命令的英文说明文本。
///
/// 参数:
/// - `command`: slash 命令文本
///
/// 返回:
/// - 适合 command palette 右侧展示的简短说明
fn command_description(command: &str) -> &'static str {
    match command {
        "/help" => "show available commands",
        "/new" => "start a new session",
        "/resume" => "resume or switch sessions",
        "/compact" => "compact older conversation history",
        "/clear" => "clear conversation; /clear memory clears memory",
        "/model" => "pick model and thinking (same as sai models)",
        "/agent" => "switch the active agent",
        "/providers" => "pick provider/model and thinking (same as /model)",
        "/config" => "open fullscreen settings",
        "/ps" => "manage background tasks",
        "/thinking" => "set reasoning effort",
        "/plan" => "switch to read-only planning mode",
        "/audit" => "switch to audited workspace sandbox mode",
        "/yolo" => "switch to YOLO mode",
        "/auto" | "/auto-audit" => "switch to auto-audit mode",
        "/goal" => "manage long-running goals",
        "/tree" => "browse the session tree and switch branches",
        "/undo" => "undo the last turn and restore input",
        "/exit" => "leave the REPL",
        _ => "",
    }
}

/// 提取斜杠菜单后面的参数文本。
///
/// 参数:
/// - `input`: 当前输入内容
/// - `command`: 斜杠菜单名称
///
/// 返回:
/// - 匹配时返回参数文本
pub(super) fn repl_command_rest<'a>(input: &'a str, command: &str) -> Option<&'a str> {
    let input = input.trim();
    if input.eq_ignore_ascii_case(command) {
        return Some("");
    }
    let rest = input.get(command.len()..)?;
    if input[..command.len()].eq_ignore_ascii_case(command)
        && rest.chars().next().is_some_and(char::is_whitespace)
    {
        return Some(rest.trim_start());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证退出命令匹配现有支持的写法，且不会误匹配相似文本。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn exit_command_matches_supported_spellings() {
        assert!(is_repl_exit_command("exit"));
        assert!(is_repl_exit_command("QUIT"));
        assert!(is_repl_exit_command(" /Exit "));
        assert!(!is_repl_exit_command("/quit"));
        assert!(!is_repl_exit_command("exit now"));
    }

    /// 自带内核下未知命令仍要拦住并提示，避免误当普通消息发出去。
    #[test]
    fn native_engine_still_reports_unknown_commands() {
        assert!(unknown_slash_command_hint("/nosuch", false).is_some());
    }

    /// 外部内核有自己的斜杠命令，必须放行交给它判断。
    ///
    /// 拦下来会让 Claude Code 的 /review 这类命令彻底发不出去——
    /// 用户看到的是「未识别的命令」，而实际上内核完全支持。
    #[test]
    fn external_engine_lets_its_own_commands_through() {
        assert!(unknown_slash_command_hint("/review", true).is_none());
        assert!(unknown_slash_command_hint("/compact", true).is_none());
    }

    /// 路径形态的输入两种内核下都按普通消息放行。
    #[test]
    fn paths_are_never_treated_as_commands() {
        assert!(unknown_slash_command_hint("/home/user/file.rs", false).is_none());
        assert!(unknown_slash_command_hint("/home/user/file.rs", true).is_none());
    }

    #[test]
    fn reset_is_not_a_repl_command() {
        assert!(!repl_commands().contains(&"/reset"));
    }

    #[test]
    fn repl_commands_include_recent_management_entries() {
        assert!(repl_commands().contains(&"/thinking"));
        assert!(repl_commands().contains(&"/ps"));
        assert!(repl_commands().contains(&"/compact"));
        assert!(repl_commands().contains(&"/model"));
        assert!(repl_commands().contains(&"/resume"));
        assert!(!repl_commands().contains(&"/帮助"));
        assert!(!repl_commands().contains(&"/压缩"));
        assert!(!repl_commands().contains(&"/模型"));
        assert!(!repl_commands().contains(&"/commands"));
        assert!(!repl_commands().contains(&"/clipb"));
        assert!(!repl_commands().contains(&"/set"));
    }

    #[test]
    fn command_rest_requires_boundary() {
        assert_eq!(
            repl_command_rest("/thinking high", "/thinking"),
            Some("high")
        );
        assert_eq!(repl_command_rest("/think", "/thinking"), None);
    }

    #[test]
    fn ps_command_completes_background_manager() {
        assert_eq!(complete_repl_command("/ps"), Some("/ps"));
    }

    #[test]
    fn suggestions_include_command_descriptions() {
        let suggestions = repl_command_suggestions("/mo");

        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].command, "/model");
        assert!(!suggestions[0].description.is_empty());
    }

    #[test]
    fn visible_suggestions_match_panel_capacity() {
        let suggestions = visible_repl_command_suggestions("/");

        assert_eq!(suggestions.len(), MAX_REPL_COMMAND_SUGGESTIONS);
        assert!(repl_command_suggestions("/").len() > suggestions.len());
    }

    #[test]
    fn command_descriptions_are_always_english() {
        let suggestions = visible_repl_command_suggestions("/");

        assert!(suggestions
            .iter()
            .all(|suggestion| suggestion.description.is_ascii()));
    }
}
/// slash 命令面板中的单条说明。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ReplCommandSuggestion {
    pub(super) command: &'static str,
    pub(super) description: &'static str,
}
pub(super) const MAX_REPL_COMMAND_SUGGESTIONS: usize = 8;
