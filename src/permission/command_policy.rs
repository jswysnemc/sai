use serde_json::Value;

/// 判断命令是否需要在工作区沙箱外执行。
///
/// 参数:
/// - `arguments`: `run_command` 的工具参数
///
/// 返回:
/// - 显式申请提升权限、包管理器、网络命令或访问工作区外路径时返回 `true`
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
        .is_some_and(command_requires_sandbox_escape)
}

/// 判断 Shell 命令是否需要沙箱外执行。
///
/// 参数:
/// - `command`: 完整 Shell 命令
///
/// 返回:
/// - 需要网络、包管理、家目录/系统路径时返回 `true`
fn command_requires_sandbox_escape(command: &str) -> bool {
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

    // 1. 常见网络客户端
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

    // 2. 包管理器 / AUR：默认沙箱会拦网络与家目录缓存
    if tokens.iter().any(|token| {
        matches!(
            command_name(token),
            "paru"
                | "yay"
                | "pacman"
                | "pikaur"
                | "trizen"
                | "pamac"
                | "apt"
                | "apt-get"
                | "aptitude"
                | "dnf"
                | "yum"
                | "zypper"
                | "apk"
                | "brew"
                | "flatpak"
                | "snap"
        )
    }) {
        return true;
    }

    // 3. 显式访问家目录或常见系统路径
    if command_touches_outside_workspace_paths(command) {
        return true;
    }

    // 4. Git 远程操作
    tokens.windows(2).any(|pair| {
        command_name(pair[0]) == "git"
            && matches!(pair[1], "clone" | "fetch" | "pull" | "push" | "ls-remote")
    })
}

/// 判断命令是否明显触碰工作区外路径。
fn command_touches_outside_workspace_paths(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    lower.contains("~/")
        || lower.contains("$home")
        || lower.contains("${home")
        || lower.contains("/home/")
        || lower.contains("/root/")
        || lower.contains("/var/lib/")
        || lower.contains("/etc/")
}

/// 清理 Shell 参数外围引号和常见标点。
fn normalize_token(token: &str) -> &str {
    token.trim_matches(|character| matches!(character, '\'' | '"' | ','))
}

/// 提取可能包含路径的命令名称。
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

    #[test]
    fn detects_package_managers() {
        assert!(requires_sandbox_escape(&json!({"command":"paru -Qua"})));
        assert!(requires_sandbox_escape(&json!({"command":"pacman -Syu"})));
        assert!(requires_sandbox_escape(&json!({"command":"apt update"})));
    }

    #[test]
    fn detects_home_path_access() {
        assert!(requires_sandbox_escape(
            &json!({"command":"cat ~/.config/sai/config.jsonc"})
        ));
    }

    #[test]
    fn keeps_local_commands_sandboxed() {
        assert!(!requires_sandbox_escape(&json!({"command":"cargo test"})));
        assert!(!requires_sandbox_escape(
            &json!({"command":"git status --short"})
        ));
    }

    #[test]
    fn accepts_explicit_escalation_request() {
        assert!(requires_sandbox_escape(&json!({
            "command":"python script.py",
            "sandbox_permissions":"require_escalated"
        })));
    }
}
