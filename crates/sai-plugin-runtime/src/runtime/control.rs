use crate::host::InvocationServices;
use crate::ExecutionLimits;
use mlua::{HookTriggers, Lua, VmState};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

/// 【插件】【执行状态】宿主能力依据 Rust 状态授权，不信任 Lua 可修改的上下文。
#[derive(Default)]
pub(super) struct CallControl {
    pub active: AtomicBool,
    pub writable: AtomicBool,
    pub generation: AtomicU64,
    pub progress_messages: AtomicUsize,
    pub model_requests: AtomicUsize,
    pub tool_calls: AtomicUsize,
    pub system_calls: AtomicUsize,
    workdir: Mutex<String>,
    session: Mutex<String>,
    pub private_mutations: AtomicBool,
    pub workspaces: Mutex<Vec<Arc<dyn crate::host::PluginWorkspace>>>,
    services: Mutex<Option<Arc<dyn InvocationServices>>>,
}

/// 【插件】【调用租约】正常返回、错误或取消时统一释放宿主服务。
pub(super) struct InvocationLease(Arc<CallControl>);

impl CallControl {
    /// 【插件】【调用开始】绑定本次服务并重置回调预算。
    /// @param services 当前任务提供的受限服务；事件回调不提供服务
    /// @returns 在作用域结束时撤销服务和调用权限的租约
    pub fn begin(
        self: &Arc<Self>,
        services: Option<Arc<dyn InvocationServices>>,
        workdir: &str,
        session: &str,
        private_mutations: bool,
    ) -> mlua::Result<InvocationLease> {
        *self
            .services
            .lock()
            .map_err(|_| mlua::Error::runtime("plugin service lock poisoned"))? = services;
        *self
            .workdir
            .lock()
            .map_err(|_| mlua::Error::runtime("plugin workdir lock poisoned"))? =
            workdir.to_string();
        *self
            .session
            .lock()
            .map_err(|_| mlua::Error::runtime("plugin session lock poisoned"))? =
            session.to_string();
        self.private_mutations
            .store(private_mutations, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.progress_messages.store(0, Ordering::Release);
        self.model_requests.store(0, Ordering::Release);
        self.tool_calls.store(0, Ordering::Release);
        self.system_calls.store(0, Ordering::Release);
        self.writable.store(false, Ordering::Release);
        self.active.store(true, Ordering::Release);
        Ok(InvocationLease(self.clone()))
    }

    /// 【插件】【服务读取】仅在当前回调有效时取得宿主服务，不持锁等待异步操作。
    /// @returns 当前服务；初始化、事件和过期调用均返回错误
    pub fn services(&self) -> mlua::Result<Arc<dyn InvocationServices>> {
        if !self.active.load(Ordering::Acquire) {
            return Err(mlua::Error::runtime(
                "host services are only available during plugin callbacks",
            ));
        }
        self.services
            .lock()
            .map_err(|_| mlua::Error::runtime("plugin service lock poisoned"))?
            .clone()
            .ok_or_else(|| mlua::Error::runtime("host services are unavailable for this callback"))
    }

    /// 【插件】【系统作用域】只从 Rust 调用状态读取工作目录和权限，忽略 Lua 表中的同名字段。
    /// @returns 本次调用的可信系统上下文；加载或结束后拒绝访问
    pub fn system_context(&self) -> mlua::Result<crate::host::SystemContext> {
        if !self.active.load(Ordering::Acquire) {
            return Err(mlua::Error::runtime(
                "system services are only available during plugin callbacks",
            ));
        }
        Ok(crate::host::SystemContext {
            workdir: self
                .workdir
                .lock()
                .map_err(|_| mlua::Error::runtime("plugin workdir lock poisoned"))?
                .clone(),
            allow_writes: self.writable.load(Ordering::Acquire),
        })
    }

    /// 【插件】【私有作用域】从宿主状态读取会话标识，事件不允许改变私有数据。
    /// @param mutation 本次操作是否写入状态或创建目录
    /// @returns 可信会话标识；加载阶段及失效调用返回错误
    pub fn private_session(&self, mutation: bool) -> mlua::Result<String> {
        self.system_context()?;
        if mutation && !self.private_mutations.load(Ordering::Acquire) {
            return Err(mlua::Error::runtime(
                "private mutations are unavailable for event callbacks",
            ));
        }
        self.session
            .lock()
            .map(|value| value.clone())
            .map_err(|_| mlua::Error::runtime("plugin session lock poisoned"))
    }
}

impl Drop for InvocationLease {
    /// 【插件】【调用结束】先撤销权限，再释放服务引用，避免取消后保留调用图。
    /// @returns 无
    fn drop(&mut self) {
        self.0.active.store(false, Ordering::Release);
        self.0.writable.store(false, Ordering::Release);
        self.0.private_mutations.store(false, Ordering::Release);
        if let Ok(mut workspaces) = self.0.workspaces.lock() {
            workspaces.clear();
        }
        if let Ok(mut services) = self.0.services.lock() {
            services.take();
        }
        if let Ok(mut workdir) = self.0.workdir.lock() {
            workdir.clear();
        }
    }
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
