use crate::ExecutionLimits;
use mlua::{HookTriggers, Lua, VmState};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

/// 【插件】【执行状态】宿主能力依据 Rust 状态授权，不信任 Lua 可修改的上下文。
#[derive(Default)]
pub(super) struct CallControl {
    pub active: AtomicBool,
    pub writable: AtomicBool,
    pub generation: AtomicU64,
    pub progress_messages: AtomicUsize,
}

/// 【插件】【指令预算】为主线程和后续异步调用线程安装统一执行限制。
/// @param lua 虚拟机；limits 为本次限制；cancel 为调用取消信号
/// @returns 安装结果；超时、取消或耗尽指令时 Lua 返回错误
pub(super) fn install_budget(
    lua: &Lua,
    limits: &ExecutionLimits,
    cancel: CancellationToken,
) -> mlua::Result<()> {
    let remaining = AtomicU64::new(limits.instructions);
    let deadline = Instant::now() + Duration::from_millis(limits.timeout_ms);
    lua.set_global_hook(
        HookTriggers::new().every_nth_instruction(1_000),
        move |_, _| {
            if cancel.is_cancelled() {
                return Err(mlua::Error::runtime("plugin execution cancelled"));
            }
            if Instant::now() >= deadline {
                return Err(mlua::Error::runtime("plugin execution timed out"));
            }
            let previous = remaining.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_sub(1_000)
            });
            if previous.is_err() {
                return Err(mlua::Error::runtime("plugin instruction budget exceeded"));
            }
            Ok(VmState::Continue)
        },
    )
}
