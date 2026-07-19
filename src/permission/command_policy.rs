use serde_json::Value;

/// 判断命令是否需要在工作区沙箱外执行。
///
/// 参数:
/// - `arguments`: `run_command` 的工具参数
///
/// 返回:
/// - 显式申请提升权限或检测到网络命令时返回 `true`
pub(super) fn requires_sandbox_escape(arguments: &Value) -> bool {
    if arguments
        .get("sandbox_permissions")
        .and_then(Value::as_str)
        .is_some_and(|value| value == "require_escalated")
    {
        return true;
    }
    arguments
        .get("command")
        .and_then(Value::as_str)
        .is_some_and(command_requires_network)
}

/// 判断 Shell 命令是否包含常见网络访问入口。
///
/// 参数:
/// - `command`: 完整 Shell 命令
///
/// 返回:
/// - 命令明确包含网络客户端或远程 Git 操作时返回 `true`
fn command_requires_network(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    if ["http://", "https://", "ftp://", "ssh://", "git://"]
        .iter()
        .any(|scheme| lower.contains(scheme))
    {
        return true;
    }

    let tokens = lower
        .split(|character: char| {
            character.is_whitespace()
                || matches!(character, '|' | '&' | ';' | '(' | ')' | '<' | '>')
        })
        .map(normalize_token)
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();

    // 1. 识别直接发起网络连接的常见程序
    if tokens.iter().any(|token| {
        matches!(
            command_name(token),
            "curl"
                | "wget"
                | "http"
                | "https"
                | "ftp"
                | "ssh"
                | "scp"
                | "sftp"
                | "telnet"
                | "nc"
                | "ncat"
                | "socat"
                | "ping"
                | "dig"
                | "nslookup"
                | "host"
        )
    }) {
        return true;
    }

    // 2. 仅将 Git 的远程子命令视为网络访问，保留本地 Git 操作的沙箱
    tokens.windows(2).any(|pair| {
        command_name(pair[0]) == "git"
            && matches!(pair[1], "clone" | "fetch" | "pull" | "push" | "ls-remote")
    })
}

/// 清理 Shell 参数外围引号和常见标点。
///
/// 参数:
/// - `token`: Shell 粗分词结果
///
/// 返回:
/// - 可用于命令匹配的参数片段
fn normalize_token(token: &str) -> &str {
    token.trim_matches(|character| matches!(character, '\'' | '"' | ','))
}

/// 提取可能包含路径的命令名称。
///
/// 参数:
/// - `token`: 命令参数片段
///
/// 返回:
/// - 去除 Unix 或 Windows 路径后的程序名称
fn command_name(token: &str) -> &str {
    token
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(token)
        .trim_end_matches(".exe")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 验证常见网络客户端会申请沙箱外执行。
    #[test]
    fn detects_network_clients() {
        assert!(requires_sandbox_escape(
            &json!({"command":"curl -fsSL example.com"})
        ));
        assert!(requires_sandbox_escape(
            &json!({"command":"git fetch origin"})
        ));
        assert!(requires_sandbox_escape(
            &json!({"command":"ssh server.example"})
        ));
    }

    /// 验证本地命令继续使用工作区沙箱。
    #[test]
    fn keeps_local_commands_sandboxed() {
        assert!(!requires_sandbox_escape(&json!({"command":"cargo test"})));
        assert!(!requires_sandbox_escape(
            &json!({"command":"git status --short"})
        ));
    }

    /// 验证复杂命令可以显式申请提升权限。
    #[test]
    fn accepts_explicit_escalation_request() {
        assert!(requires_sandbox_escape(&json!({
            "command":"python script.py",
            "sandbox_permissions":"require_escalated"
        })));
    }
}
