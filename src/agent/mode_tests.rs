use super::{AgentMode, LiveAgentMode, PermissionMode};
use crate::permission::PermissionProfileMode;
use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc,
};

/// 【会话模式】【兼容回归】无参数；验证旧值保留审批且所有组合均可往返存储。
#[test]
fn legacy_and_combined_modes_round_trip() {
    assert_eq!(
        AgentMode::parse(Some("code-model")).unwrap(),
        AgentMode::CodeModel
    );
    assert_eq!(AgentMode::CodeModel.permission(), PermissionMode::Audited);
    for permission in [
        AgentMode::Yolo,
        AgentMode::Audited,
        AgentMode::AutoAudit,
        AgentMode::Plan,
    ] {
        for enabled in [false, true] {
            let mode = permission.with_codemode(enabled);
            assert_eq!(AgentMode::parse(Some(mode.key())).unwrap(), mode);
            assert_eq!(
                mode.permission_profile_mode(),
                permission.permission_profile_mode()
            );
            assert_eq!(mode.reminder().contains("name=\"code-model\""), enabled);
            assert!(mode.label().contains(permission.label()));
        }
    }
    assert!(AgentMode::parse(Some("yolo+invalid")).is_err());
}

/// 【会话模式】【独立切换】无参数；验证工具开关与权限命令互不覆盖。
#[test]
fn commands_and_cycle_preserve_the_other_dimension() {
    let enabled = AgentMode::Yolo.command("/code-model").unwrap();
    assert_eq!(enabled.permission(), PermissionMode::Yolo);
    assert!(enabled.uses_codemode());
    let audited = enabled.command("/audit").unwrap();
    assert!(audited.uses_codemode());
    assert_eq!(audited.permission(), PermissionMode::Audited);
    assert_eq!(audited.command("/code-model off"), Some(AgentMode::Audited));
    assert_eq!(enabled.command("/code-model"), Some(AgentMode::Yolo));
    assert_eq!(enabled.command("/code-model on"), Some(enabled));
    assert_eq!(enabled.next().next().next(), enabled);
    assert_eq!(enabled.next().permission(), PermissionMode::Audited);
}

/// 【会话模式】【实时权限】无参数；验证共享注册表权限保持独立并能实时反映外部更新。
#[test]
fn live_tools_never_change_permission_encoding() {
    let permission = Arc::new(AtomicU8::new(0));
    let mode = AgentMode::Yolo.with_codemode(true);
    let live = LiveAgentMode::new(mode, Some(permission.clone()));
    assert_eq!(live.get(), mode);
    assert_eq!(
        permission.load(Ordering::SeqCst),
        PermissionProfileMode::Yolo.as_u8()
    );
    permission.store(PermissionProfileMode::AutoAudit.as_u8(), Ordering::SeqCst);
    assert_eq!(live.get(), AgentMode::AutoAudit.with_codemode(true));
    live.set(AgentMode::Audited);
    assert_eq!(live.get(), AgentMode::Audited);
}
