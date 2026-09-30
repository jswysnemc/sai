//! 在真实 bubblewrap 中验证隔离效果；本机无法建立沙箱时跳过。

use super::policy::{FileAccess, SandboxPolicy};
use super::settings::SandboxSettings;
use crate::config::SandboxConfig;
use crate::tools::command::run_shell_command;

/// 本机 bwrap 不可用时返回 `false`，测试直接跳过。
fn backend_ready() -> bool {
    super::backend_availability().available
}

/// 在工作区内按指定配置执行命令。
async fn run_in(
    workspace: &std::path::Path,
    config: SandboxConfig,
    access: FileAccess,
    command: &str,
) -> std::process::Output {
    let settings = SandboxSettings {
        config,
        protected_files: Vec::new(),
    };
    let policy = SandboxPolicy::resolve(&settings, access, workspace, None);
    run_shell_command(command, 10, "sh", Some(&policy))
        .await
        .unwrap()
}

/// 验证只允许写入工作区与私有临时目录。
#[tokio::test]
async fn blocks_writes_outside_workspace() {
    if !backend_ready() {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let output = run_in(
        &workspace,
        SandboxConfig::default(),
        FileAccess::WorkspaceWrite,
        "touch allowed && touch \"$TMPDIR/scratch\" && touch ../blocked",
    )
    .await;
    assert!(!output.status.success());
    assert!(workspace.join("allowed").exists());
    assert!(!root.path().join("blocked").exists());
    let stderr = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();
    assert!(
        super::detect_denial("", &stderr, false).is_some(),
        "{stderr}"
    );
}

/// 验证只读策略连工作区也不能写。
#[tokio::test]
async fn read_only_policy_blocks_workspace_writes() {
    if !backend_ready() {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().canonicalize().unwrap();
    let output = run_in(
        &workspace,
        SandboxConfig::default(),
        FileAccess::ReadOnly,
        "ls >/dev/null && touch plan-write",
    )
    .await;
    assert!(!output.status.success());
    assert!(!workspace.join("plan-write").exists());
}

/// 验证隐藏路径读不到内容，Git 钩子目录不可写。
#[tokio::test]
async fn hides_denied_paths_and_protects_git_hooks() {
    if !backend_ready() {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().canonicalize().unwrap();
    std::fs::create_dir_all(workspace.join(".git/hooks")).unwrap();
    std::fs::create_dir_all(workspace.join("vault")).unwrap();
    std::fs::write(workspace.join("vault/key"), "top-secret").unwrap();
    std::fs::write(workspace.join(".env"), "API_KEY=1").unwrap();
    let config = SandboxConfig {
        deny_read: vec!["vault".into(), ".env".into()],
        ..SandboxConfig::default()
    };
    let output = run_in(
        &workspace,
        config,
        FileAccess::WorkspaceWrite,
        "cat vault/key .env 2>/dev/null; touch .git/hooks/pre-commit; echo done",
    )
    .await;
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("done"));
    assert!(!stdout.contains("top-secret"));
    assert!(!stdout.contains("API_KEY"));
    assert!(!workspace.join(".git/hooks/pre-commit").exists());
    assert!(workspace.join("vault/key").exists());
}

/// 验证沙箱标记与私有临时目录写入环境。
#[tokio::test]
async fn exports_sandbox_marker_and_private_tmpdir() {
    if !backend_ready() {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let output = run_in(
        root.path(),
        SandboxConfig::default(),
        FileAccess::WorkspaceWrite,
        "printf '%s|%s' \"$SAI_SANDBOX\" \"$TMPDIR\"",
    )
    .await;
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.starts_with("workspace_write|"), "{stdout}");
    assert!(stdout.contains("sai-sandbox-"), "{stdout}");
}
