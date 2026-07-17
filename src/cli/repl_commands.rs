/// 返回 REPL 支持的斜杠菜单。
///
/// 返回:
/// - 斜杠菜单列表
pub(super) fn repl_commands() -> &'static [&'static str] {
    crate::control_commands::catalog::REPL_COMMANDS
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
    repl_commands()
        .iter()
        .copied()
        .filter(|command| command.starts_with(input))
        .map(|command| ReplCommandSuggestion {
            command,
            description: command_description(command),
        })
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

/// 返回 slash 命令在当前语言下的说明文本。
///
/// 参数:
/// - `command`: slash 命令文本
///
/// 返回:
/// - 适合 command palette 右侧展示的简短说明
fn command_description(command: &str) -> &'static str {
    let zh = is_zh();
    match command {
        "/help" => {
            if zh {
                "显示可用命令"
            } else {
                "show available commands"
            }
        }
        "/new" => {
            if zh {
                "创建新会话"
            } else {
                "start a new session"
            }
        }
        "/resume" => {
            if zh {
                "恢复或切换会话"
            } else {
                "resume or switch sessions"
            }
        }
        "/compact" => {
            if zh {
                "压缩较早的会话内容"
            } else {
                "compact older conversation history"
            }
        }
        "/clear" => {
            if zh {
                "清空会话；/clear memory 清空记忆"
            } else {
                "clear conversation; /clear memory clears memory"
            }
        }
        "/model" => {
            if zh {
                "选择当前模型"
            } else {
                "choose the active model"
            }
        }
        "/agent" => {
            if zh {
                "切换当前 Agent"
            } else {
                "switch the active agent"
            }
        }
        "/providers" => {
            if zh {
                "切换服务商或模型"
            } else {
                "switch provider or model"
            }
        }
        "/config" => {
            if zh {
                "打开配置界面"
            } else {
                "open configuration"
            }
        }
        "/ps" => {
            if zh {
                "管理后台任务"
            } else {
                "manage background tasks"
            }
        }
        "/thinking" => {
            if zh {
                "设置思考强度"
            } else {
                "set reasoning effort"
            }
        }
        "/plan" => {
            if zh {
                "切换到只读规划模式"
            } else {
                "switch to read-only planning mode"
            }
        }
        "/audit" => {
            if zh {
                "切换到权限审计和工作区沙盒模式"
            } else {
                "switch to audited workspace sandbox mode"
            }
        }
        "/yolo" => {
            if zh {
                "切换到 YOLO 模式"
            } else {
                "switch to YOLO mode"
            }
        }
        "/undo" => {
            if zh {
                "撤销上一轮并恢复输入"
            } else {
                "undo the last turn and restore input"
            }
        }
        "/exit" => {
            if zh {
                "退出 REPL"
            } else {
                "leave the REPL"
            }
        }
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
}
use crate::i18n::is_zh;

/// slash 命令面板中的单条说明。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ReplCommandSuggestion {
    pub(super) command: &'static str,
    pub(super) description: &'static str,
}
