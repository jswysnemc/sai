use crate::ExecutionLimits;
use mlua::{HookTriggers, Lua, VmState};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

/// 【插件】【共用预算】Lua 指令和原生纯计算共用可信额度、截止时间及取消信号
struct ExecutionBudget {
    remaining: AtomicU64,
    deadline: Instant,
    cancel: CancellationToken,
}

impl ExecutionBudget {
    /// 【插件】【预算扣除】先检查有效期，再原子扣除指令或字节计算额度
    /// @param units 本次消耗，零表示仅检查时间及取消状态
    /// @returns 超时、取消或额度不足时返回错误
    fn charge(&self, units: u64) -> mlua::Result<()> {
        if self.cancel.is_cancelled() {
            return Err(mlua::Error::runtime("plugin execution cancelled"));
        }
        if Instant::now() >= self.deadline {
            return Err(mlua::Error::runtime("plugin execution timed out"));
        }
        self.remaining
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_sub(units)
            })
            .map_err(|_| mlua::Error::runtime("plugin instruction budget exceeded"))?;
        Ok(())
    }
}

/// 【插件】【指令预算】加载及每次回调重置预算，Lua 钩子与宿主函数引用同一份状态
/// @param lua 虚拟机；limits 为本次限制；cancel 为调用取消信号
/// @returns 安装结果；内部状态不能通过 Lua 修改
pub(super) fn install(
    lua: &Lua,
    limits: &ExecutionLimits,
    cancel: CancellationToken,
) -> mlua::Result<()> {
    let budget = Arc::new(ExecutionBudget {
        remaining: AtomicU64::new(limits.instructions),
        deadline: Instant::now() + Duration::from_millis(limits.timeout_ms),
        cancel,
    });
    lua.set_app_data(budget.clone());
    lua.set_global_hook(
        HookTriggers::new().every_nth_instruction(1000),
        move |_, _| {
            budget.charge(1000)?;
            Ok(VmState::Continue)
        },
    )
}

/// 【插件】【原生计算】每个输入字节消耗一单位预算，空输入也至少消耗一单位
/// @param lua 当前虚拟机；bytes 为本次输入字节数量
/// @returns 预算扣除结果
pub(super) fn charge_bytes(lua: &Lua, bytes: usize) -> mlua::Result<()> {
    current(lua)?.charge(bytes.max(1) as u64)
}

/// 【插件】【计算检查】在分块计算和交付结果之前检查超时与取消状态
/// @param lua 当前虚拟机
/// @returns 当前调用仍有效时成功
pub(super) fn checkpoint(lua: &Lua) -> mlua::Result<()> {
    current(lua)?.charge(0)
}

/// 【插件】【可信状态】从 Rust 虚拟机附加数据取得当前预算，不读取公开的 sai.limits
/// @param lua 当前虚拟机
/// @returns 当前预算；未安装预算时拒绝原生计算
fn current(lua: &Lua) -> mlua::Result<Arc<ExecutionBudget>> {
    lua.app_data_ref::<Arc<ExecutionBudget>>()
        .map(|budget| Arc::clone(&budget))
        .ok_or_else(|| mlua::Error::runtime("plugin execution budget unavailable"))
}
