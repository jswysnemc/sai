use super::*;
use crate::tools::command::test_support::isolated_test_paths;

/// 验证只读命令允许查询类操作。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn readonly_command_allows_inspection() {
    assert!(ensure_readonly_command("git status --short").is_ok());
    assert!(ensure_readonly_command("pacman -Q sai").is_ok());
}

/// 验证只读命令拒绝写入和执行类操作。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn readonly_command_blocks_mutation() {
    assert!(ensure_readonly_command("rm file").is_err());
    assert!(ensure_readonly_command("sed -i 's/a/b/' file").is_err());
    assert!(ensure_readonly_command("cargo test").is_err());
    assert!(ensure_readonly_command("Remove-Item file").is_err());
    assert!(ensure_readonly_command("Set-Content file value").is_err());
    assert!(ensure_readonly_command("winget install foo").is_err());
}

/// 验证前台等待时间允许零值并限制最大值。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn foreground_wait_accepts_zero() {
    assert_eq!(foreground_wait_seconds(&json!({})), 30);
    assert_eq!(foreground_wait_seconds(&json!({"timeout_seconds": 0})), 0);
    assert_eq!(
        foreground_wait_seconds(&json!({"timeout_seconds": 200})),
        120
    );
}

/// 验证只读命令使用当前平台外壳执行。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[tokio::test]
async fn readonly_command_runs_with_platform_shell() {
    #[cfg(windows)]
    let command = "Write-Output hello";
    #[cfg(not(windows))]
    let command = "printf hello";

    let result = run_readonly_command(json!({"command": command}), String::new())
        .await
        .unwrap();
    let data: Value = serde_json::from_str(&result).unwrap();
    assert_eq!(data["mode"], "foreground");
    assert_eq!(data["success"], true);
    assert_eq!(data["stdout"], "hello");
}

/// 验证可写命令超时后转为后台任务并保持进程运行。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[tokio::test]
async fn writable_command_promotes_to_background_on_timeout() {
    let temp = tempfile::tempdir().unwrap();
    let paths = isolated_test_paths(temp.path().to_path_buf());
    let mut config = AppConfig::default();
    config.tools.background_commands_enabled = true;
    config.tools.background_command_timeout_seconds = 0;
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    // 托管 spawn 与前台命令统一优先使用 PowerShell；这里使用两种 Shell 都支持的命令。
    #[cfg(windows)]
    let command = "ping -n 6 127.0.0.1 >nul";
    #[cfg(not(windows))]
    let command = "printf 'before\n'; sleep 5";

    let result = run_command(
        json!({"command": command, "timeout_seconds": 1, "label": "promote-test"}),
        true,
        String::new(),
        ToolProgress::new(sender),
        &config,
        &paths,
        None,
    )
    .await
    .unwrap();
    let data: Value = serde_json::from_str(&result).unwrap();
    assert_eq!(
        data["mode"], "background",
        "expected background promotion, got: {data}"
    );
    assert_eq!(data["promoted"], true);
    let task_id = data["task_id"].as_str().unwrap().to_string();
    let pid = data["task"]["pid"].as_u64().unwrap() as u32;
    assert!(
        process_exists(pid),
        "promoted task pid should still be alive"
    );

    // 清理：停止后台进程
    terminate_process(pid, data["task"]["pgid"].as_i64().map(|v| v as i32), true).await;
    let _ = receiver.try_recv();
    assert!(!task_id.is_empty());
}

/// 验证可写命令及时结束时返回前台结果并清理任务记录。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[tokio::test]
async fn writable_command_returns_foreground_when_finished() {
    let temp = tempfile::tempdir().unwrap();
    let paths = isolated_test_paths(temp.path().to_path_buf());
    let mut config = AppConfig::default();
    config.tools.background_commands_enabled = true;
    let (sender, _receiver) = tokio::sync::mpsc::unbounded_channel();
    // 该断言走托管 spawn 路径，使用 PowerShell 专有命令可防止后台路径悄悄回到 cmd。
    #[cfg(windows)]
    let command = "Write-Output done";
    #[cfg(not(windows))]
    let command = "printf 'done\n'";

    let result = run_command(
        json!({"command": command, "timeout_seconds": 10}),
        true,
        String::new(),
        ToolProgress::new(sender),
        &config,
        &paths,
        None,
    )
    .await
    .unwrap();
    let data: Value = serde_json::from_str(&result).unwrap();
    assert_eq!(
        data["mode"], "foreground",
        "expected foreground finish, got: {data}"
    );
    let stdout = data["stdout"].as_str().unwrap_or_default();
    assert!(
        stdout.to_ascii_lowercase().contains("done"),
        "stdout should contain done, got: {stdout:?}"
    );
    assert_eq!(data["completed"], true);
    assert_eq!(data["success"], true);
    assert_eq!(data["exit_code"], 0);
    let task_id = data["task_id"]
        .as_str()
        .expect("foreground managed finish keeps audit task_id");
    let note = data["note"].as_str().unwrap_or_default();
    assert!(
        note.contains("audit-only"),
        "foreground note should mark task_id as audit-only, got: {note}"
    );
    let store = BackgroundCommandStore::new(paths.state_dir.clone());
    let tasks = store.load().unwrap();
    assert!(
        tasks.iter().all(|item| item.id != task_id),
        "foreground finished tasks should be auto-removed after output is consumed"
    );
}

/// 验证前台托管命令回传真实非零退出码。
#[tokio::test]
async fn writable_command_reports_nonzero_exit_code() {
    let temp = tempfile::tempdir().unwrap();
    let paths = isolated_test_paths(temp.path().to_path_buf());
    let mut config = AppConfig::default();
    config.tools.background_commands_enabled = true;
    let (sender, _receiver) = tokio::sync::mpsc::unbounded_channel();
    #[cfg(windows)]
    let command = "cmd /c exit 7";
    #[cfg(not(windows))]
    let command = "exit 7";

    let result = run_command(
        json!({"command": command, "timeout_seconds": 10}),
        true,
        String::new(),
        ToolProgress::new(sender),
        &config,
        &paths,
        None,
    )
    .await
    .unwrap();
    let data: Value = serde_json::from_str(&result).unwrap();
    assert_eq!(data["mode"], "foreground");
    assert_eq!(data["completed"], true);
    assert_eq!(data["success"], false);
    assert_eq!(data["exit_code"], 7);
}
