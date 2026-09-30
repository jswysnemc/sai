use super::policy::{FileAccess, SandboxPolicy};
use std::fmt::Write as _;
use std::path::Path;

/// macOS 自带的 Seatbelt 启动器，使用绝对路径避免被 PATH 劫持。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";

/// 基础规则：默认拒绝，放行进程管理、系统信息读取与常见系统服务。
/// 规则取自 Chrome 与 sandbox-runtime 的 Seatbelt 策略，均为 sandbox-exec 可解析语法。
const BASE_PROFILE: &str = r#"(version 1)
(deny default)
(allow process-exec)
(allow process-fork)
(allow process-info* (target same-sandbox))
(allow signal (target same-sandbox))
(allow mach-priv-task-port (target same-sandbox))
(allow user-preference-read)
(allow mach-lookup
  (global-name "com.apple.distributed_notifications@Uv3")
  (global-name "com.apple.FontObjectsServer")
  (global-name "com.apple.fonts")
  (global-name "com.apple.logd")
  (global-name "com.apple.lsd.mapdb")
  (global-name "com.apple.system.logger")
  (global-name "com.apple.system.notification_center")
  (global-name "com.apple.system.opendirectoryd.libinfo")
  (global-name "com.apple.system.opendirectoryd.membership")
  (global-name "com.apple.bsd.dirhelper")
  (global-name "com.apple.securityd.xpc")
  (global-name "com.apple.SecurityServer")
  (global-name "com.apple.trustd.agent")
  (global-name "com.apple.coreservices.launchservicesd"))
(allow ipc-posix-shm)
(allow ipc-posix-sem)
(allow iokit-open
  (iokit-registry-entry-class "IOSurfaceRootUserClient")
  (iokit-registry-entry-class "RootDomainUserClient")
  (iokit-user-client-class "IOSurfaceSendRight"))
(allow iokit-get-properties)
(allow system-socket (require-all (socket-domain AF_SYSTEM) (socket-protocol 2)))
(allow sysctl-read)
(allow distributed-notification-post)
(allow pseudo-tty)
(allow file-ioctl
  (literal "/dev/null")
  (literal "/dev/zero")
  (literal "/dev/random")
  (literal "/dev/urandom")
  (literal "/dev/dtracehelper")
  (literal "/dev/tty")
  (literal "/dev/ptmx")
  (regex #"^/dev/ttys"))
(allow file-read* file-write*
  (literal "/dev/null")
  (literal "/dev/stdout")
  (literal "/dev/stderr")
  (literal "/dev/tty")
  (literal "/dev/dtracehelper")
  (literal "/dev/ptmx")
  (regex #"^/dev/ttys"))
(allow file-read*)
"#;

/// 【沙箱】【Seatbelt】生成 sandbox-exec 规则文本。Seatbelt 按最后匹配生效，
/// 因此先放行可写目录，再拒绝保护子路径与隐藏路径。
///
/// 参数:
/// - `policy`: 已解析策略
///
/// 返回:
/// - 规则文本
pub(crate) fn seatbelt_profile(policy: &SandboxPolicy) -> String {
    let mut profile = String::from(BASE_PROFILE);
    // 1. 可写目录；只读模式下只有私有临时目录
    let writable = policy
        .writable_roots
        .iter()
        .map(|root| subpath(root))
        .collect::<Vec<_>>();
    if !writable.is_empty() {
        let _ = writeln!(profile, "(allow file-write*\n  {})", writable.join("\n  "));
    }
    // 2. 可写目录中的只读子路径；Seatbelt 能拦截尚不存在的路径
    if !policy.write_protected.is_empty() {
        let protected = policy
            .write_protected
            .iter()
            .map(|path| subpath(path))
            .collect::<Vec<_>>();
        let _ = writeln!(profile, "(deny file-write*\n  {})", protected.join("\n  "));
    }
    // 3. 隐藏路径同时禁止读取与写入
    if !policy.deny_read.is_empty() {
        let denied = policy
            .deny_read
            .iter()
            .map(|path| subpath(path))
            .collect::<Vec<_>>();
        let _ = writeln!(
            profile,
            "(deny file-read* file-write*\n  {})",
            denied.join("\n  ")
        );
    }
    // 4. 网络：允许时放行全部出入站，拒绝时只保留本机 Unix 域套接字
    if policy.network {
        profile.push_str("(allow network*)\n(allow system-socket)\n");
    } else {
        profile.push_str(UNIX_SOCKET_RULES);
    }
    if policy.access == FileAccess::ReadOnly {
        profile.push_str(";; read-only planning sandbox\n");
    }
    profile
}

/// 断网时仍允许本机 Unix 域套接字，供 git、ssh-agent 等本地通信使用。
const UNIX_SOCKET_RULES: &str = r#"(allow system-socket (socket-domain AF_UNIX))
(allow network-bind (local unix-socket (path-regex #"^/")))
(allow network-outbound (remote unix-socket (path-regex #"^/")))
"#;

/// 生成 subpath 过滤器，转义双引号与反斜杠。
fn subpath(path: &Path) -> String {
    let text = path.display().to_string();
    format!(
        "(subpath \"{}\")",
        text.replace('\\', "\\\\").replace('"', "\\\"")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// 构造测试策略。
    fn policy(network: bool, access: FileAccess) -> SandboxPolicy {
        SandboxPolicy {
            access,
            cwd: PathBuf::from("/w"),
            writable_roots: vec![PathBuf::from("/w"), PathBuf::from("/tmp/sai \"x\"")],
            write_protected: vec![PathBuf::from("/w/.git/hooks")],
            deny_read: vec![PathBuf::from("/Users/me/.ssh")],
            network,
            temp_dir: None,
            env: Vec::new(),
            scrubbed_env: Vec::new(),
        }
    }

    /// 验证规则顺序满足最后匹配生效：放行在前、拒绝在后。
    #[test]
    fn profile_orders_allow_before_deny() {
        let text = seatbelt_profile(&policy(false, FileAccess::WorkspaceWrite));
        let allow = text.find("(allow file-write*").unwrap();
        let hooks = text
            .find("(deny file-write*\n  (subpath \"/w/.git/hooks\")")
            .unwrap();
        let ssh = text.find("(subpath \"/Users/me/.ssh\")").unwrap();
        assert!(text.starts_with("(version 1)\n(deny default)"));
        assert!(allow < hooks && hooks < ssh);
        assert!(text.contains("(subpath \"/tmp/sai \\\"x\\\"\")"));
        assert!(text.contains("(allow system-socket (socket-domain AF_UNIX))"));
        assert!(!text.contains("(allow network*)\n"));
    }

    /// 验证允许网络与只读模式标记。
    #[test]
    fn profile_allows_network_when_configured() {
        let text = seatbelt_profile(&policy(true, FileAccess::ReadOnly));
        assert!(text.contains("(allow network*)\n(allow system-socket)"));
        assert!(text.contains("read-only planning sandbox"));
    }
}
