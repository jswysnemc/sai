use super::policy::SandboxPolicy;
use std::ffi::OsString;

/// 【沙箱】【bubblewrap】生成 bwrap 参数，挂载顺序决定覆盖关系：
/// 只读根 → 可写目录 → 只读保护子路径 → 隐藏路径。
///
/// 参数:
/// - `policy`: 已解析策略
/// - `shell`: 沙箱内使用的 Shell
/// - `command`: Shell 命令文本
///
/// 返回:
/// - 传给 `bwrap` 的参数列表
pub(crate) fn bwrap_args(policy: &SandboxPolicy, shell: &str, command: &str) -> Vec<OsString> {
    let mut args: Vec<OsString> = Vec::new();
    let mut push =
        |items: &[&std::ffi::OsStr]| args.extend(items.iter().map(|item| item.to_os_string()));
    // 1. 进程隔离：父进程退出即终止，新会话防止 TIOCSTI 注入终端
    push(&[
        "--die-with-parent".as_ref(),
        "--new-session".as_ref(),
        "--unshare-pid".as_ref(),
    ]);
    if !policy.network {
        push(&["--unshare-net".as_ref()]);
    }
    // 2. 整个根目录只读，再挂新的 /dev 与 /proc
    push(&["--ro-bind".as_ref(), "/".as_ref(), "/".as_ref()]);
    push(&[
        "--dev".as_ref(),
        "/dev".as_ref(),
        "--proc".as_ref(),
        "/proc".as_ref(),
    ]);
    // 3. 可写目录
    for root in policy.writable_roots.iter().filter(|root| root.exists()) {
        push(&["--bind".as_ref(), root.as_os_str(), root.as_os_str()]);
    }
    // 4. 可写目录中仍需只读的子路径；bwrap 只能保护已存在路径
    for path in policy.write_protected.iter().filter(|path| path.exists()) {
        push(&["--ro-bind".as_ref(), path.as_os_str(), path.as_os_str()]);
    }
    // 5. 隐藏凭据：目录以空 tmpfs 覆盖，文件以 /dev/null 覆盖
    for path in &policy.deny_read {
        if path.is_dir() {
            push(&["--tmpfs".as_ref(), path.as_os_str()]);
        } else if path.exists() {
            push(&["--ro-bind".as_ref(), "/dev/null".as_ref(), path.as_os_str()]);
        }
    }
    push(&["--chdir".as_ref(), policy.cwd.as_os_str()]);
    push(&[
        "--".as_ref(),
        shell.as_ref(),
        "-lc".as_ref(),
        command.as_ref(),
    ]);
    args
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sandbox::policy::FileAccess;
    use std::path::PathBuf;

    /// 构造测试策略。
    fn policy(root: &std::path::Path, network: bool) -> SandboxPolicy {
        let secret = root.join("secret");
        std::fs::create_dir_all(&secret).unwrap();
        std::fs::write(root.join("token"), "x").unwrap();
        std::fs::create_dir_all(root.join(".git/hooks")).unwrap();
        SandboxPolicy {
            access: FileAccess::WorkspaceWrite,
            cwd: root.to_path_buf(),
            writable_roots: vec![root.to_path_buf(), PathBuf::from("/missing/root")],
            write_protected: vec![root.join(".git/hooks"), root.join("missing/hooks")],
            deny_read: vec![secret, root.join("token"), root.join("absent")],
            network,
            temp_dir: None,
            env: Vec::new(),
            scrubbed_env: Vec::new(),
        }
    }

    /// 把参数拼成空格分隔字符串便于断言。
    fn joined(args: &[OsString]) -> String {
        args.iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// 验证挂载顺序与缺失路径跳过规则。
    #[test]
    fn builds_ordered_mounts() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path().canonicalize().unwrap();
        let text = joined(&bwrap_args(&policy(&root, false), "sh", "echo hi"));
        let r = root.display();
        let ro_root = text.find("--ro-bind / /").unwrap();
        let bind = text.find(&format!("--bind {r} {r}")).unwrap();
        let hooks = text.find(&format!("--ro-bind {r}/.git/hooks")).unwrap();
        let tmpfs = text.find(&format!("--tmpfs {r}/secret")).unwrap();
        assert!(ro_root < bind && bind < hooks && hooks < tmpfs);
        assert!(text.contains(&format!("--ro-bind /dev/null {r}/token")));
        assert!(text.contains("--unshare-net"));
        assert!(!text.contains("/missing"));
        assert!(!text.contains("absent"));
        assert!(text.ends_with("-- sh -lc echo hi"));
    }

    /// 验证允许网络时不再断开网络命名空间。
    #[test]
    fn network_allow_keeps_host_network() {
        let root = tempfile::tempdir().unwrap();
        let text = joined(&bwrap_args(&policy(root.path(), true), "sh", "true"));
        assert!(!text.contains("--unshare-net"));
        assert!(text.contains("--unshare-pid"));
    }
}
