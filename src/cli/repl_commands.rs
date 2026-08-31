use crate::i18n::text as t;

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

/// 模型运行期间斜杠命令的执行策略。
///
/// 运行期间 `&mut Agent` 被轮次独占，交互选择器也会占住事件循环，
/// 因此按是否触碰这两者把命令分为立即执行、置灰与立即退出三档。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::cli) enum StreamCommandPolicy {
    /// 不触碰 Agent 状态、不弹交互选择器，可在本轮流式期间同步执行
    Immediate,
    /// 需要 `&mut Agent`、交互选择器或 `.await`，只能等本轮结束后执行
    Disabled,
    /// 退出 REPL，无需等待本轮结束
    Exit,
    /// 不是控制命令，按普通消息处理
    NotCommand,
}

/// `parse_control_command` 未覆盖、由主循环用字符串比较单独分发的命令。
///
/// 判定控制命令时必须单独覆盖它们，否则运行期间输入 `/plan` 之类会被
/// 解析器判为普通文本，混进消息队列当成提问发给模型。
const EXTRA_REPL_COMMANDS: &[&str] = &[
    "/plan",
    "/audit",
    "/yolo",
    "/auto",
    "/auto-audit",
    "/providers",
    "/config",
    "/undo",
    "/thinking",
    "/ps",
];

/// 其中只切换权限模式的命令：等价于 Shift+Tab 热切换，可立即生效。
const STREAM_IMMEDIATE_EXTRA_COMMANDS: &[&str] =
    &["/plan", "/audit", "/yolo", "/auto", "/auto-audit"];

/// 判断输入在模型运行期间的执行策略。
///
/// 参数:
/// - `input`: 已输入或提交的原始文本
///
/// 返回:
/// - 立即执行 / 置灰 / 立即退出 / 非控制命令
pub(in crate::cli) fn stream_command_policy(input: &str) -> StreamCommandPolicy {
    let input = input.trim();
    if input.is_empty() {
        return StreamCommandPolicy::NotCommand;
    }
    if is_repl_exit_command(input) {
        return StreamCommandPolicy::Exit;
    }
    if input.starts_with('!') {
        // shell 命令要 await 子进程，同步路径下无法执行
        return StreamCommandPolicy::Disabled;
    }
    if let Some(policy) = extra_stream_command_policy(input) {
        return policy;
    }
    match crate::control_commands::parse_control_command(
        input,
        crate::control_commands::ControlSurface::Repl,
    ) {
        // 只读查询：自建 StateStore 或读子智能体目录，不触碰 Agent
        Ok(Some(
            crate::control_commands::ControlCommand::Help
            | crate::control_commands::ControlCommand::Context { .. }
            | crate::control_commands::ControlCommand::Rename { .. }
            | crate::control_commands::ControlCommand::Subagents
            | crate::control_commands::ControlCommand::SubagentMessage { .. },
        )) => StreamCommandPolicy::Immediate,
        Ok(Some(_)) => StreamCommandPolicy::Disabled,
        _ => StreamCommandPolicy::NotCommand,
    }
}

/// 判断主循环单独分发的命令的执行策略。
///
/// 参数:
/// - `input`: 已去除首尾空白的输入
///
/// 返回:
/// - 命中这类命令时返回其策略；否则为空
fn extra_stream_command_policy(input: &str) -> Option<StreamCommandPolicy> {
    for &command in EXTRA_REPL_COMMANDS {
        if repl_command_rest(input, command).is_some() {
            return Some(if STREAM_IMMEDIATE_EXTRA_COMMANDS.contains(&command) {
                StreamCommandPolicy::Immediate
            } else {
                StreamCommandPolicy::Disabled
            });
        }
    }
    None
}

/// 判断输入是否需要主循环以控制命令方式执行。
///
/// 斜杠命令、shell 前缀与退出命令都无法作为聊天正文发给模型，
/// 运行期间输入时必须与消息队列分流。
///
/// 参数:
/// - `input`: 已输入或提交的原始文本
///
/// 返回:
/// - 需要交主循环分发时返回 true
pub(in crate::cli) fn is_stream_command_text(input: &str) -> bool {
    !matches!(
        stream_command_policy(input),
        StreamCommandPolicy::NotCommand
    )
}

/// 生成运行期间命中置灰命令的提示文本。
///
/// 参数:
/// - `input`: 被拒绝的命令原文
///
/// 返回:
/// - 说明该命令需等本轮结束的提示
pub(super) fn stream_command_disabled_hint(input: &str) -> String {
    let name = input.trim().split_whitespace().next().unwrap_or_default();
    if crate::i18n::is_zh() {
        format!("{name} 需等本轮结束后执行；已保留在输入框，本轮结束后按 Enter 即可")
    } else {
        format!("{name} runs after this turn; kept in the input box — press Enter once it finishes")
    }
}

/// 根据当前输入生成斜杠菜单补全建议。
///
/// 参数:
/// - `input`: 当前输入内容
/// - `streaming`: 模型是否正在运行；为 true 时把打断类命令标记为禁用
///
/// 返回:
/// - 可补全的斜杠菜单
pub(super) fn repl_command_suggestions(input: &str, streaming: bool) -> Vec<ReplCommandSuggestion> {
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
            disabled: streaming
                && matches!(
                    stream_command_policy(command),
                    StreamCommandPolicy::Disabled
                ),
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
/// - `streaming`: 模型是否正在运行
///
/// 返回:
/// - 不超过面板容量的补全建议
pub(super) fn visible_repl_command_suggestions(
    input: &str,
    streaming: bool,
) -> Vec<ReplCommandSuggestion> {
    repl_command_suggestions(input, streaming)
        .into_iter()
        .take(MAX_REPL_COMMAND_SUGGESTIONS)
        .collect()
}

/// 返回唯一匹配的斜杠菜单补全文本。
///
/// 参数:
/// - `input`: 当前输入内容
/// - `streaming`: 模型是否正在运行；为 true 时不补全置灰命令
///
/// 返回:
/// - 唯一补全结果
pub(super) fn complete_repl_command(input: &str, streaming: bool) -> Option<&'static str> {
    let suggestions: Vec<ReplCommandSuggestion> = repl_command_suggestions(input, streaming)
        .into_iter()
        .filter(|suggestion| !suggestion.disabled)
        .collect();
    if suggestions.len() == 1 {
        suggestions.first().map(|suggestion| suggestion.command)
    } else {
        None
    }
}

/// 返回 slash 命令的本地化说明文本。
///
/// 参数:
/// - `command`: slash 命令文本
///
/// 返回:
/// - 适合 command palette 右侧展示的简短说明
fn command_description(command: &str) -> &'static str {
    match command {
        "/help" => t("show available commands", "显示可用命令"),
        "/context" => t(
            "show context usage and compaction policy",
            "查看上下文占用与压缩策略",
        ),
        "/new" => t("start a new session", "新建会话"),
        "/resume" => t("resume or switch sessions", "恢复或切换会话"),
        "/rename" => t("rename the current session", "为当前会话命名"),
        "/compact" => t("compact older conversation history", "压缩旧对话历史"),
        "/clear" => t(
            "clear conversation; /clear memory clears memory",
            "清空对话；/clear memory 同时清空记忆",
        ),
        "/model" => t(
            "pick model and thinking (same as sai models)",
            "选择模型与思考等级（与 sai models 相同）",
        ),
        "/agent" => t("switch the active agent", "切换当前 Agent"),
        "/providers" => t(
            "pick provider/model and thinking (same as /model)",
            "选择供应商、模型与思考等级（与 /model 相同）",
        ),
        "/config" => t("open fullscreen settings", "打开全屏配置"),
        "/ps" => t("manage background tasks", "管理后台任务"),
        "/subagents" => t("list session subagents", "列出会话子智能体"),
        "/msg" => t("leave a message on a subagent", "给子智能体留言"),
        "/thinking" => t("set reasoning effort", "设置思考等级"),
        "/plan" => t("switch to read-only planning mode", "切换到只读计划模式"),
        "/audit" => t(
            "switch to audited workspace sandbox mode",
            "切换到审核工作区沙箱模式",
        ),
        "/yolo" => t("switch to YOLO mode", "切换到 YOLO 模式"),
        "/auto" | "/auto-audit" => t("switch to auto-audit mode", "切换到自动审核模式"),
        "/goal" => t("manage long-running goals", "管理长期目标"),
        "/tree" => t(
            "browse the session tree and switch branches",
            "浏览会话树并切换分支",
        ),
        "/undo" => t(
            "undo the last turn and restore input",
            "撤销上一轮并恢复输入",
        ),
        "/exit" => t("leave the REPL", "退出 REPL"),
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
        assert!(repl_commands().contains(&"/rename"));
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
        assert_eq!(complete_repl_command("/ps", false), Some("/ps"));
    }

    #[test]
    fn suggestions_include_command_descriptions() {
        let suggestions = repl_command_suggestions("/mo", false);

        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].command, "/model");
        assert!(!suggestions[0].description.is_empty());
    }

    #[test]
    fn visible_suggestions_match_panel_capacity() {
        let suggestions = visible_repl_command_suggestions("/", false);

        assert_eq!(suggestions.len(), MAX_REPL_COMMAND_SUGGESTIONS);
        assert!(repl_command_suggestions("/", false).len() > suggestions.len());
    }

    #[test]
    fn command_descriptions_follow_locale() {
        let suggestions = visible_repl_command_suggestions("/", false);

        assert!(suggestions
            .iter()
            .all(|suggestion| !suggestion.description.is_empty()));
        if crate::i18n::is_zh() {
            assert!(suggestions
                .iter()
                .any(|suggestion| !suggestion.description.is_ascii()));
        } else {
            assert!(suggestions
                .iter()
                .all(|suggestion| suggestion.description.is_ascii()));
        }
    }

    /// 只读查询命令在模型运行期间立即执行，不必等本轮结束。
    #[test]
    fn read_only_commands_run_immediately() {
        for command in ["/context", "/subagents", "/help", "/rename Demo"] {
            assert_eq!(
                stream_command_policy(command),
                StreamCommandPolicy::Immediate,
                "{command} should run immediately"
            );
        }
    }

    /// 权限模式切换等价于 Shift+Tab 热切换，运行期间同样立即生效。
    #[test]
    fn mode_switch_commands_run_immediately() {
        for command in ["/plan", "/audit", "/yolo", "/auto", "/auto-audit"] {
            assert_eq!(
                stream_command_policy(command),
                StreamCommandPolicy::Immediate,
                "{command} should switch mode immediately"
            );
        }
    }

    /// 需要 Agent 状态或交互选择器的命令运行期间置灰。
    #[test]
    fn stateful_commands_are_disabled_while_streaming() {
        for command in [
            "/new",
            "/resume",
            "/compact",
            "/clear",
            "/clear memory",
            "/model",
            "/agent",
            "/providers",
            "/config",
            "/tree",
            "/thinking",
            "/ps",
            "/undo",
            "!ls",
        ] {
            assert_eq!(
                stream_command_policy(command),
                StreamCommandPolicy::Disabled,
                "{command} should be disabled while streaming"
            );
        }
    }

    /// 退出命令任何时候都立即退出，不等本轮结束。
    #[test]
    fn exit_command_exits_immediately() {
        assert_eq!(stream_command_policy("/exit"), StreamCommandPolicy::Exit);
        assert_eq!(stream_command_policy("exit"), StreamCommandPolicy::Exit);
        assert_eq!(stream_command_policy("QUIT"), StreamCommandPolicy::Exit);
    }

    /// 主循环单独分发的命令也必须判定为控制命令。
    ///
    /// 漏判会让它们在运行期间混进消息队列，被当成提问发给模型。
    #[test]
    fn extra_commands_are_control_commands() {
        for command in [
            "/plan",
            "/audit",
            "/yolo",
            "/auto",
            "/auto-audit",
            "/providers",
            "/config",
            "/undo",
            "/thinking high",
            "/ps",
        ] {
            assert!(
                is_stream_command_text(command),
                "{command} must be treated as a control command"
            );
        }
    }

    /// 普通消息与未知斜杠输入都不是控制命令。
    #[test]
    fn plain_text_is_not_a_control_command() {
        assert_eq!(
            stream_command_policy("write a quicksort"),
            StreamCommandPolicy::NotCommand
        );
        assert!(!is_stream_command_text(""));
        assert!(!is_stream_command_text("/nosuchcommand"));
    }

    /// 运行期间的补全跳过置灰命令，避免补全到一条不能执行的命令。
    #[test]
    fn disabled_commands_are_not_completed_while_streaming() {
        assert_eq!(complete_repl_command("/ps", false), Some("/ps"));
        assert_eq!(complete_repl_command("/ps", true), None);
        assert_eq!(complete_repl_command("/con", true), Some("/context"));
    }

    /// 置灰命令的提示要带上命令名，让用户知道被拒绝的是哪一条。
    #[test]
    fn disabled_hint_names_the_command() {
        let hint = stream_command_disabled_hint("/model");
        assert!(hint.contains("/model"), "{hint}");
    }
}
/// slash 命令面板中的单条说明。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ReplCommandSuggestion {
    pub(super) command: &'static str,
    pub(super) description: &'static str,
    /// 模型运行期间是否不可用（需要 Agent 状态或交互选择器）
    pub(super) disabled: bool,
}
pub(super) const MAX_REPL_COMMAND_SUGGESTIONS: usize = 8;
