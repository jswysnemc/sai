use super::{AgentMode, PermissionMode};
use crate::permission::PermissionProfileMode;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

/// 共享权限句柄和独立工具开关，避免脚本模式改变注册表的权限策略。
pub struct LiveAgentMode {
    permission: Arc<AtomicU8>,
    codemode: AtomicU8,
}

impl LiveAgentMode {
    /// 【会话模式】【状态绑定】参数 mode 为初始模式，permission 为注册表权限句柄；返回共享状态。
    pub(crate) fn new(mode: AgentMode, permission: Option<Arc<AtomicU8>>) -> Arc<Self> {
        let state = Arc::new(Self {
            permission: permission.unwrap_or_else(|| Arc::new(AtomicU8::new(0))),
            codemode: AtomicU8::new(0),
        });
        state.set(mode);
        state
    }

    /// 【会话模式】【状态读取】无参数；返回最新权限策略与工具方式。
    pub fn get(&self) -> AgentMode {
        let permission =
            match PermissionProfileMode::from_u8(self.permission.load(Ordering::SeqCst)) {
                PermissionProfileMode::Yolo => PermissionMode::Yolo,
                PermissionProfileMode::Audited | PermissionProfileMode::CodeModel => {
                    PermissionMode::Audited
                }
                PermissionProfileMode::AutoAudit => PermissionMode::AutoAudit,
                PermissionProfileMode::Plan => PermissionMode::Plan,
            };
        let tools = match self.codemode.load(Ordering::SeqCst) {
            1 => super::ToolMode::CodeModel,
            2 => super::ToolMode::CodeModelOnly,
            _ => super::ToolMode::Native,
        };
        AgentMode::new(permission, tools)
    }

    /// 【会话模式】【状态更新】参数 mode 为目标组合；分别更新权限句柄与工具开关，返回无。
    pub fn set(&self, mode: AgentMode) {
        self.permission
            .store(mode.permission_profile_mode().as_u8(), Ordering::SeqCst);
        let tools = match mode.codemode() {
            crate::config::CodeMode::Off => 0,
            crate::config::CodeMode::On => 1,
            crate::config::CodeMode::Only => 2,
        };
        self.codemode.store(tools, Ordering::SeqCst);
    }

    /// 【会话模式】【权限共享】无参数；返回仅包含权限策略的注册表句柄。
    pub(crate) fn permission_handle(&self) -> Arc<AtomicU8> {
        self.permission.clone()
    }

    /// 【会话模式】【注册表替换】参数 registry 为新注册表，mode 为目标模式；保留界面共享句柄，返回无。
    pub(crate) fn bind_registry(&self, registry: &mut crate::tools::ToolRegistry, mode: AgentMode) {
        if let Some(profile) = registry.permission_profile() {
            registry.set_permission_profile(profile.with_mode_handle(self.permission_handle()));
        }
        self.set(mode);
    }
}
