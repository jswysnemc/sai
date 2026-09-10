use crate::host::InvocationServices;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

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
    pub binary_buffers: Mutex<Vec<std::sync::Weak<crate::host::binary::BinarySlot>>>,
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
        if let Ok(mut buffers) = self.0.binary_buffers.lock() {
            for buffer in buffers.drain(..).filter_map(|buffer| buffer.upgrade()) {
                if let Ok(mut data) = buffer.lock() {
                    data.take();
                }
            }
        }
        if let Ok(mut services) = self.0.services.lock() {
            services.take();
        }
        if let Ok(mut workdir) = self.0.workdir.lock() {
            workdir.clear();
        }
    }
}
