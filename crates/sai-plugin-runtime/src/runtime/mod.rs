mod bindings;
mod control;
mod execution;
mod http;
mod modules;
mod registration;
mod services;
mod text;

use crate::host::{InvocationServices, PluginHost};
use crate::{
    Capabilities, EventContext, EventKind, PluginCommand, PluginManifest, PluginPackage, PluginTool,
};
use anyhow::{bail, Context, Result};
use mlua::chunk::ChunkMode;
use mlua::{Lua, LuaOptions, LuaSerdeExt, StdLib, Value as LuaValue};
use registration::{RegisteredCommand, RegisteredTool, Registrations};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

pub type ProgressCallback = Arc<dyn Fn(String) + Send + Sync>;

static NEXT_INSTANCE_ID: AtomicU64 = AtomicU64::new(1);

/// 【插件】【调用上下文】宿主明确交付的会话信息与当前操作权限。
#[derive(Clone, Default)]
pub struct InvocationContext {
    pub session_id: String,
    pub workdir: String,
    pub allow_writes: bool,
    pub progress: Option<ProgressCallback>,
    pub services: Option<Arc<dyn InvocationServices>>,
}

/// 【插件】【运行实例】独立 Lua 状态与全部注册的共同所有者。
#[derive(Clone)]
pub struct PluginRuntime {
    instance_id: u64,
    manifest: Arc<PluginManifest>,
    tools: Arc<Vec<PluginTool>>,
    commands: Arc<Vec<PluginCommand>>,
    events: Arc<Vec<EventKind>>,
    vm: Arc<Mutex<Vm>>,
}

struct Vm {
    lua: Lua,
    tools: BTreeMap<String, RegisteredTool>,
    commands: BTreeMap<String, RegisteredCommand>,
    events: BTreeMap<EventKind, Vec<mlua::Function>>,
    control: Arc<control::CallControl>,
}

enum Invocation {
    Tool(String, Value),
    Command(String, String),
    Event(EventKind, Value),
}

impl PluginRuntime {
    /// 【插件】【加载事务】构造独立虚拟机，执行入口后一次性提交所有注册。
    /// @param package 源码快照；settings 为插件自己的配置；granted 为宿主授权；host 为宿主能力实现
    /// @returns 完整插件实例，任一步骤失败时不对外留下注册
    pub fn load(
        package: PluginPackage,
        settings: Value,
        granted: Capabilities,
        host: Arc<dyn PluginHost>,
    ) -> Result<Self> {
        package.manifest.validate()?;
        granted.validate()?;
        let manifest = Arc::new(package.manifest.clone());
        let capabilities = package.manifest.capabilities.intersection(&granted);
        let lua = Lua::new_with(
            StdLib::TABLE | StdLib::STRING | StdLib::MATH | StdLib::UTF8,
            LuaOptions::default(),
        )?;
        lua.set_memory_limit(manifest.limits.memory_bytes)?;
        // 1. 加载阶段仅允许纯计算和注册，宿主 I/O 在调用阶段才开放
        let control = Arc::new(control::CallControl::default());
        control::install_budget(&lua, &manifest.limits, CancellationToken::new())?;
        let api = lua.create_table()?;
        api.set("config", lua.to_value(&settings)?)?;
        api.set("plugin_id", manifest.id.as_str())?;
        api.set("limits", lua.to_value(&manifest.limits)?)?;
        bindings::install(
            &lua,
            &api,
            host,
            capabilities,
            manifest.limits.clone(),
            control.clone(),
        )?;
        modules::install(&lua, &package)?;
        let registrations = Arc::new(StdMutex::new(Registrations::default()));
        registration::install(&lua, &api, registrations.clone())?;
        lua.globals().set("sai", api)?;
        // 【插件】【加载隔离】异步绑定会加载 coroutine 库，必须在绑定完成后移除全局入口
        for name in [
            "dofile",
            "loadfile",
            "load",
            "collectgarbage",
            "print",
            "coroutine",
        ] {
            lua.globals().set(name, LuaValue::Nil)?;
        }
        lua.load(&package.sources[&manifest.entry])
            .set_mode(ChunkMode::Text)
            .set_name(format!("@{}/{}", manifest.id, manifest.entry))
            .exec()
            .with_context(|| format!("load Lua plugin {}", manifest.id))?;
        // 2. 转移函数所有权后封闭加载事务，禁止回调中留下动态注册
        let mut registrations = registrations
            .lock()
            .map_err(|_| anyhow::anyhow!("plugin registration lock poisoned"))?;
        let tools = std::mem::take(&mut registrations.tools);
        let commands = std::mem::take(&mut registrations.commands);
        let events = std::mem::take(&mut registrations.events);
        registrations.closed = true;
        let metadata = tools.values().map(|tool| tool.definition.clone()).collect();
        let command_metadata = commands
            .values()
            .map(|command| command.definition.clone())
            .collect();
        let event_metadata = events.keys().copied().collect();
        drop(registrations);
        Ok(Self {
            instance_id: NEXT_INSTANCE_ID.fetch_add(1, Ordering::Relaxed),
            manifest,
            tools: Arc::new(metadata),
            commands: Arc::new(command_metadata),
            events: Arc::new(event_metadata),
            vm: Arc::new(Mutex::new(Vm {
                lua,
                tools,
                commands,
                events,
                control,
            })),
        })
    }

    /// 返回插件清单的只读引用，无参数。
    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    /// 【插件】【实例标识】供宿主识别共享同一 VM 的运行时克隆，避免嵌套事件重入。
    /// @returns 进程内实例标识；克隆保留标识，重新加载生成新标识，不用于持久化
    pub fn instance_id(&self) -> u64 {
        self.instance_id
    }

    /// 返回已经提交的工具定义，无参数。
    pub fn tools(&self) -> &[PluginTool] {
        &self.tools
    }

    /// 返回已经提交的用户命令，无参数。
    pub fn commands(&self) -> &[PluginCommand] {
        &self.commands
    }

    /// 返回当前插件监听的事件，无参数。
    pub fn events(&self) -> &[EventKind] {
        &self.events
    }

    /// 【插件】【工具执行】验证参数后调用 Lua 工具，保留插件实例内的状态。
    /// @param name 包内工具名；arguments 为参数对象；context 为宿主上下文
    /// @returns 文本或格式化 JSON 结果
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: Value,
        context: InvocationContext,
    ) -> Result<String> {
        output_text(
            self.invoke(Invocation::Tool(name.to_string(), arguments), context)
                .await?,
        )
    }

    /// 【插件】【命令执行】执行用户命令，宿主能力仍由本次调用上下文约束。
    /// @param name 包内命令名；arguments 为用户参数；context 为宿主上下文
    /// @returns 命令显示文本
    pub async fn call_command(
        &self,
        name: &str,
        arguments: &str,
        context: InvocationContext,
    ) -> Result<String> {
        output_text(
            self.invoke(
                Invocation::Command(name.to_string(), arguments.to_string()),
                context,
            )
            .await?,
        )
    }

    /// 【插件】【事件分发】顺序调用当前插件注册的监听器。
    /// @param event 事件类型；context 为事件及会话资料
    /// @returns 按注册顺序排列的监听器返回值；事件回调没有写入授权
    pub async fn emit(&self, event: EventKind, context: EventContext) -> Result<Vec<Value>> {
        if !self.events.contains(&event) {
            return Ok(Vec::new());
        }
        let invocation_context = InvocationContext {
            session_id: context.session_id,
            workdir: context.workdir,
            ..Default::default()
        };
        let value = self
            .invoke(Invocation::Event(event, context.data), invocation_context)
            .await?;
        serde_json::from_value(value).context("invalid plugin event results")
    }

    /// 【插件】【受限调度】把 Lua 运算放到阻塞工作线程，取消后同步停止异步宿主调用。
    /// @param invocation 调用目标；context 为宿主上下文
    /// @returns 资源限制内的 JSON 返回值
    async fn invoke(&self, invocation: Invocation, context: InvocationContext) -> Result<Value> {
        let cancel = CancellationToken::new();
        let _cancel_on_drop = cancel.clone().drop_guard();
        let vm = self.vm.clone();
        let limits = self.manifest.limits.clone();
        let timeout = Duration::from_millis(limits.timeout_ms);
        let operation = async move {
            let mut vm = vm.lock_owned().await;
            let handle = tokio::runtime::Handle::current();
            tokio::task::spawn_blocking(move || {
                handle.block_on(async move {
                control::install_budget(&vm.lua, &limits, cancel.clone())?;
                let result = tokio::select! {
                    _ = cancel.cancelled() => Err(anyhow::anyhow!("plugin execution cancelled")),
                    result = vm.execute(invocation, context) => result,
                };
                vm.control.active.store(false, std::sync::atomic::Ordering::Release);
                let value = result?;
                if serde_json::to_vec(&value)?.len() > limits.output_bytes {
                    bail!("plugin output exceeds size limit");
                }
                Ok::<_, anyhow::Error>(value)
            })
            })
            .await
            .context("plugin execution worker failed")?
        };
        tokio::time::timeout(timeout, operation)
            .await
            .context("plugin execution timed out")?
            .with_context(|| format!("plugin {} callback failed", self.manifest.id))
    }
}

/// 【插件】【输出适配】字符串直接显示，其他结构化结果序列化为 JSON。
/// @param value Lua 返回的 JSON 值
/// @returns 可交给 Sai 工具结果通道的文本
fn output_text(value: Value) -> Result<String> {
    match value {
        Value::String(text) => Ok(text),
        Value::Null => Ok(String::new()),
        value => Ok(serde_json::to_string_pretty(&value)?),
    }
}
