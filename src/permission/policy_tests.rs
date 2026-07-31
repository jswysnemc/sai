use super::*;
use serde_json::json;

/// 验证未经批准的审计模式仍阻止工作区外的显式路径。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn audited_profile_blocks_explicit_path_outside_workspace() {
    let profile = PermissionProfile::new(
        PermissionProfileMode::Audited,
        PathBuf::from("/workspace/project"),
        None,
    );
    assert!(profile
        .authorize(
            "edit_file",
            ToolPermission::Writes,
            &json!({"patch":"*** Begin Patch\n*** Update File: ../secret\n@@\n-old\n+new\n*** End Patch"})
        )
        .is_err());
}

/// 验证批准后允许一次工作区外写入。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn audited_profile_allows_approved_external_write_once() {
    let profile = PermissionProfile::new(
        PermissionProfileMode::Audited,
        PathBuf::from("/workspace/project"),
        None,
    );
    let args = json!({"patch":"*** Begin Patch\n*** Update File: ../secret\n@@\n-old\n+new\n*** End Patch"});
    profile.record_approved("edit_file", &args, None);
    assert!(profile
        .authorize("edit_file", ToolPermission::Writes, &args)
        .is_ok());
    assert!(profile
        .authorize("edit_file", ToolPermission::Writes, &args)
        .is_err());
}

/// 验证工作区外读取需要交互批准。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn audited_profile_requires_approval_for_any_external_read() {
    let profile = PermissionProfile::new(
        PermissionProfileMode::Audited,
        PathBuf::from("/workspace/project"),
        None,
    );
    let args = json!({"path":"/home/user/notes.txt"});
    assert!(profile.requires_interactive_audit("read_file", ToolPermission::ReadOnly, &args));
    assert!(profile
        .authorize("read_file", ToolPermission::ReadOnly, &args)
        .is_err());
    profile.record_approved("read_file", &args, None);
    assert!(profile
        .authorize("read_file", ToolPermission::ReadOnly, &args)
        .is_ok());
}

/// 验证后台命令在审计模式下批准后可以运行。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn audited_profile_allows_approved_background_command() {
    let profile = PermissionProfile::new(
        PermissionProfileMode::Audited,
        PathBuf::from("/workspace/project"),
        None,
    );
    let args = json!({"action":"start", "command":"sleep 1"});
    profile.record_approved("background_command", &args, None);
    assert!(profile
        .authorize("background_command", ToolPermission::Writes, &args)
        .is_ok());
}

/// 验证 YOLO 模式保持不受限制的兼容行为。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn yolo_profile_keeps_unrestricted_behavior() {
    let profile = PermissionProfile::new(
        PermissionProfileMode::Yolo,
        PathBuf::from("/workspace/project"),
        None,
    );
    assert!(!profile
        .authorize(
            "edit_file",
            ToolPermission::Writes,
            &json!({"patch":"*** Begin Patch\n*** Update File: /etc/hosts\n@@\n-old\n+new\n*** End Patch"})
        )
        .unwrap());
}

/// 验证审计模式阻止 Patch 移动到工作区外。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn audited_profile_blocks_patch_destination_outside_workspace() {
    let profile = PermissionProfile::new(
        PermissionProfileMode::Audited,
        PathBuf::from("/workspace/project"),
        None,
    );
    let patch = "*** Begin Patch\n*** Update File: src/main.rs\n*** Move to: ../escaped.rs\n@@\n-old\n+new\n*** End Patch";
    assert!(profile
        .authorize("edit_file", ToolPermission::Writes, &json!({"patch":patch}))
        .is_err());
}

/// 验证 TODO 工具不需要交互式权限审计。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn audited_profile_skips_todo_audit() {
    let profile = PermissionProfile::new(
        PermissionProfileMode::Audited,
        PathBuf::from("/workspace/project"),
        None,
    );
    assert!(!profile.requires_interactive_audit(
        "todo",
        ToolPermission::Writes,
        &json!({"action":"add", "text":"检查"}),
    ));
}

/// 验证审计模式仅在 Linux 请求命令沙箱。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn audited_run_command_only_requests_linux_sandbox() {
    let profile = PermissionProfile::new(
        PermissionProfileMode::Audited,
        PathBuf::from("/workspace/project"),
        None,
    );

    let sandboxed = profile
        .authorize(
            "run_command",
            ToolPermission::Writes,
            &json!({"command":"printf ok"}),
        )
        .unwrap();

    assert_eq!(sandboxed, cfg!(target_os = "linux"));
}

/// 验证用户批准网络命令后不再隔离网络命名空间。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn audited_profile_runs_approved_network_command_outside_sandbox() {
    let profile = PermissionProfile::new(
        PermissionProfileMode::Audited,
        PathBuf::from("/workspace/project"),
        None,
    );
    let args = json!({"command":"curl https://example.com"});

    profile.record_approved("run_command", &args, None);

    assert!(!profile
        .authorize("run_command", ToolPermission::Writes, &args)
        .unwrap());
}

/// 验证普通工作区读取不需要审计，但工作区内凭据文件仍需审计。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn audited_profile_skips_workspace_read_audit_but_catches_credentials() {
    let profile = PermissionProfile::new(
        PermissionProfileMode::Audited,
        PathBuf::from("/workspace/project"),
        None,
    );
    assert!(!profile.requires_interactive_audit(
        "read_file",
        ToolPermission::ReadOnly,
        &json!({"path":"src/main.rs"}),
    ));
    assert!(profile.requires_interactive_audit(
        "read_file",
        ToolPermission::ReadOnly,
        &json!({"path":".env.local"}),
    ));
}

/// 验证读取系统敏感文件需要交互式权限审计。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn audited_profile_requires_sensitive_read_audit() {
    let profile = PermissionProfile::new(
        PermissionProfileMode::Audited,
        PathBuf::from("/workspace/project"),
        None,
    );
    assert!(profile.requires_interactive_audit(
        "read_file",
        ToolPermission::ReadOnly,
        &json!({"path":"/etc/hosts"}),
    ));
    assert!(profile.requires_interactive_audit(
        "read_file",
        ToolPermission::ReadOnly,
        &json!({"files":[{"path":"src/lib.rs"}, {"path":"/etc/passwd"}]}),
    ));
    assert!(profile.requires_interactive_audit(
        "read_file",
        ToolPermission::ReadOnly,
        &json!({"path":"~/.ssh/id_rsa"}),
    ));
}

#[cfg(unix)]
/// 验证审计模式阻止通过符号链接逃逸工作区。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn audited_profile_blocks_symlink_escape() {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace");
    let outside = root.path().join("outside");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    symlink(&outside, workspace.join("linked")).unwrap();
    let profile = PermissionProfile::new(PermissionProfileMode::Audited, workspace, None);
    assert!(profile
        .authorize(
            "edit_file",
            ToolPermission::Writes,
            &json!({"patch":"*** Begin Patch\n*** Update File: linked/escaped.txt\n@@\n-old\n+new\n*** End Patch"}),
        )
        .is_err());
}

/// 验证自动审计模式要求用户批准软件包管理命令。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn auto_audit_profile_requires_package_manager_approval() {
    let workspace = std::env::temp_dir().join("sai-policy-auto-pkg");
    let _ = std::fs::create_dir_all(&workspace);
    let profile = PermissionProfile::new(PermissionProfileMode::AutoAudit, workspace, None);
    let args = serde_json::json!({"command":"paru -Qua"});
    assert!(profile.requires_interactive_audit(
        "run_command",
        crate::tools::ToolPermission::Writes,
        &args
    ));
    let err = profile
        .authorize("run_command", crate::tools::ToolPermission::Writes, &args)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("elevated sandbox") || err.contains("interactive approval"),
        "{err}"
    );
}
