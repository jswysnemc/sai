use serde_json::Value;

/// 命令需要离开工作区沙箱的原因。
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) enum EscapeReason {
    /// 模型显式申请 `require_escalated`
    Requested,
    /// 需要网络：URL、网络客户端或 Git 远程操作
    Network,
    /// 系统包管理器
    PackageManager,
    /// 访问家目录或系统路径
    OutsidePath,
}

impl EscapeReason {
    /// 返回界面与审核使用的稳定标识。
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::Network => "network",
            Self::PackageManager => "package_manager",
            Self::OutsidePath => "outside_path",
        }
    }
}

/// 判断命令是否需要在工作区沙箱外执行，网络规则按当前沙箱配置取舍。
///
/// 参数:
/// - `arguments`: `run_command` 的工具参数
///
/// 返回:
/// - 显式申请提升权限、包管理器、网络命令或访问工作区外路径时返回 `true`
pub(super) fn requires_sandbox_escape(arguments: &Value) -> bool {
    !sandbox_escape_reasons(arguments, crate::sandbox::network_allowed()).is_empty()
}

/// 按指定网络策略判断命令是否需要在沙箱外执行。
///
/// 参数:
/// - `arguments`: `run_command` 的工具参数
/// - `network_allowed`: 沙箱是否保留网络；保留时网络命令无需提升
///
/// 返回:
/// - 需要提升时返回 `true`
#[cfg(test)]
pub(super) fn requires_sandbox_escape_with(arguments: &Value, network_allowed: bool) -> bool {
    !sandbox_escape_reasons(arguments, network_allowed).is_empty()
}

/// 【权限】【沙箱提升】列出命令需要离开工作区沙箱的全部原因。
///
/// 参数:
/// - `arguments`: `run_command` 的工具参数
/// - `network_allowed`: 沙箱是否保留网络
///
/// 返回:
/// - 去重后的原因列表；为空表示可在沙箱内执行
pub(crate) fn sandbox_escape_reasons(
    arguments: &Value,
    network_allowed: bool,
) -> Vec<EscapeReason> {
    let mut reasons = Vec::new();
    if arguments
        .get("sandbox_permissions")
        .and_then(Value::as_str)
        .is_some_and(|value| value == "require_escalated")
    {
        reasons.push(EscapeReason::Requested);
    }
    if let Some(command) = arguments.get("command").and_then(Value::as_str) {
        for reason in command_escape_reasons(command, network_allowed) {
            if !reasons.contains(&reason) {
                reasons.push(reason);
            }
        }
    }
    reasons
}

/// 判断 Shell 命令需要沙箱外执行的原因。
///
/// 参数:
/// - `command`: 完整 Shell 命令
/// - `network_allowed`: 沙箱是否保留网络
///
/// 返回:
/// - 网络、包管理、家目录/系统路径原因
fn command_escape_reasons(command: &str, network_allowed: bool) -> Vec<EscapeReason> {
    let lower = command.to_ascii_lowercase();
    let tokens = lower
        .split(|character: char| {
            character.is_whitespace()
                || matches!(character, '|' | '&' | ';' | '(' | ')' | '<' | '>')
        })
        .map(normalize_token)
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    let mut reasons = Vec::new();

    // 1. 网络：URL、常见网络客户端与 Git 远程操作；沙箱保留网络时无需提升
    let has_url = ["http://", "https://", "ftp://", "ssh://", "git://"]
        .iter()
        .any(|scheme| lower.contains(scheme));
    let has_client = tokens.iter().any(|token| {
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
    });
    let git_remote = tokens.windows(2).any(|pair| {
        command_name(pair[0]) == "git"
            && matches!(pair[1], "clone" | "fetch" | "pull" | "push" | "ls-remote")
    });
    if !network_allowed && (has_url || has_client || git_remote) {
        reasons.push(EscapeReason::Network);
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
        reasons.push(EscapeReason::PackageManager);
    }

    // 3. 显式访问家目录或常见系统路径
    if command_touches_outside_workspace_paths(command) {
        reasons.push(EscapeReason::OutsidePath);
    }
    reasons
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
    fn network_allowed_sandbox_skips_network_rules_only() {
        let fetch = json!({"command":"curl -fsSL https://example.com && git fetch origin"});
        assert!(requires_sandbox_escape_with(&fetch, false));
        assert!(!requires_sandbox_escape_with(&fetch, true));
        assert!(requires_sandbox_escape_with(
            &json!({"command":"apt update"}),
            true
        ));
        assert!(requires_sandbox_escape_with(
            &json!({"command":"cat ~/.bashrc"}),
            true
        ));
    }

    #[test]
    fn lists_every_escape_reason_once() {
        let reasons = sandbox_escape_reasons(
            &json!({
                "command": "curl https://x.dev | sudo apt install -y foo > /etc/foo",
                "sandbox_permissions": "require_escalated"
            }),
            false,
        );
        assert_eq!(
            reasons,
            vec![
                EscapeReason::Requested,
                EscapeReason::Network,
                EscapeReason::PackageManager,
                EscapeReason::OutsidePath
            ]
        );
        assert!(sandbox_escape_reasons(&json!({"command":"cargo test"}), false).is_empty());
    }

    #[test]
    fn accepts_explicit_escalation_request() {
        assert!(requires_sandbox_escape(&json!({
            "command":"python script.py",
            "sandbox_permissions":"require_escalated"
        })));
    }
}
