use super::discovery::PluginDescriptor;
use anyhow::{bail, Context, Result};
use sai_plugin_runtime::host::PluginHost;
use sai_plugin_runtime::{
    EventContext, EventKind, InvocationContext, PluginCommand, PluginRuntime,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::Arc;

/// 【插件】【实例所有权】源码和注册属于同一实例，克隆仅用于同一 Agent 的工具副本。
#[derive(Clone)]
pub(super) struct PluginInstance {
    pub descriptor: PluginDescriptor,
    pub runtime: PluginRuntime,
    revision: String,
    host: Arc<dyn PluginHost>,
}

/// 【插件】【会话状态】同一 Agent 共享实例；新 Agent 必须显式 fork。
#[derive(Clone, Default)]
pub(crate) struct PluginSession {
    instances: Arc<BTreeMap<String, PluginInstance>>,
}

impl PluginInstance {
    /// 【插件】【实例加载】将描述、授权和宿主组合为完整加载事务。
    /// @param descriptor 已发现插件；host 为可替换的宿主能力实现
    /// @returns 注册完整的实例，不执行任何初始化网络请求
    pub(super) fn load(descriptor: PluginDescriptor, host: Arc<dyn PluginHost>) -> Result<Self> {
        let revision = descriptor.revision()?;
        let runtime = PluginRuntime::load(
            descriptor.runtime_package(),
            descriptor.settings().clone(),
            descriptor.grants(),
            host.clone(),
        )?;
        // 【插件】【名称校验】全包验证后才允许把任一工具交给注册表
        for tool in runtime.tools() {
            descriptor.tool_name(&tool.name)?;
        }
        Ok(Self {
            descriptor,
            runtime,
            revision,
            host,
        })
    }

    /// 【插件】【实例隔离】基于固定源码建立新 VM，禁止初始化改变已公开的契约。
    /// @returns 独立实例；工具、命令或事件元数据不稳定时返回错误
    fn fork(&self) -> Result<Self> {
        let fresh = Self::load(self.descriptor.clone(), self.host.clone())?;
        let original = serde_json::to_value((
            self.runtime.tools(),
            self.runtime.commands(),
            self.runtime.events(),
        ))?;
        let current = serde_json::to_value((
            fresh.runtime.tools(),
            fresh.runtime.commands(),
            fresh.runtime.events(),
        ))?;
        if original != current {
            bail!(
                "plugin {} registrations must be deterministic",
                self.runtime.manifest().id
            );
        }
        Ok(fresh)
    }
}

impl PluginSession {
    /// 【插件】【实例定位】读取当前包对应的 VM 标识，不获取执行锁。
    /// @param id 插件标识
    /// @returns 进程内实例标识；未加载插件返回错误
    pub(crate) fn instance_id(&self, id: &str) -> Result<u64> {
        Ok(self.runtime(id)?.instance_id())
    }

    /// 【插件】【观察者隔离】旧注册表可能持有正在执行的 VM，只重建这些实例供嵌套事件使用。
    /// @param active 当前调用链内正在执行的实例标识
    /// @returns 可安全分发事件的会话，其他插件继续共享原有状态
    pub(crate) fn fork_active(&self, active: &[u64]) -> Result<Self> {
        let mut session = self.clone();
        for (id, instance) in self.instances.iter() {
            if active.contains(&instance.runtime.instance_id()) {
                Arc::make_mut(&mut session.instances).insert(id.clone(), instance.fork()?);
            }
        }
        Ok(session)
    }

    /// 【插件】【有效能力】取得当前固定实例的声明与授权交集。
    /// @param id 当前实例标识
    /// @returns 实际生效的模型、工具与网络能力
    pub(crate) fn capabilities(&self, id: &str) -> Result<sai_plugin_runtime::Capabilities> {
        let instance = self
            .instances
            .get(id)
            .with_context(|| format!("plugin instance missing: {id}"))?;
        Ok(instance
            .descriptor
            .capabilities()
            .intersection(&instance.descriptor.grants()))
    }

    /// 【插件】【事件查询】没有监听器时跳过事件载荷复制与序列化。
    /// @param event 事件类型
    /// @returns 是否存在相应监听器
    pub(crate) fn listens(&self, event: EventKind) -> bool {
        self.instances
            .values()
            .any(|instance| instance.runtime.events().contains(&event))
    }

    /// 【插件】【运行目录】枚举当前会话真正加载的插件，不读取磁盘配置。
    /// @returns 按 ID 排序的插件标识与版本
    pub(crate) fn active_plugins(&self) -> Vec<(String, String)> {
        self.instances
            .iter()
            .map(|(id, instance)| (id.clone(), instance.runtime.manifest().version.clone()))
            .collect()
    }

    /// 【插件】【注册提交】把已校验实例加入当前会话集合。
    /// @param instance 完整插件实例
    /// @returns 无；调用方已完成跨插件名称冲突检查
    pub(super) fn insert(&mut self, instance: PluginInstance) {
        Arc::make_mut(&mut self.instances).insert(instance.runtime.manifest().id.clone(), instance);
    }

    /// 【插件】【会话隔离】为新 Agent 重新构造各插件的 VM。
    /// @returns 不共享 Lua 全局变量和监听器状态的新会话
    pub(crate) fn fork(&self) -> Result<Self> {
        let mut fresh = Self::default();
        for instance in self.instances.values() {
            fresh.insert(instance.fork()?);
        }
        Ok(fresh)
    }

    /// 【插件】【会话延续】配置和源码未变时沿用同一 Agent 的状态。
    /// @param previous 替换前的会话插件集合
    /// @returns 无；已禁用插件和已更改版本不会被重新加入
    pub(crate) fn preserve_from(&mut self, previous: &Self) {
        for (id, instance) in Arc::make_mut(&mut self.instances) {
            if let Some(old) = previous
                .instances
                .get(id)
                .filter(|old| old.revision == instance.revision)
            {
                *instance = old.clone();
            }
        }
    }

    /// 【插件】【工具复制】复制工具时同时保留其完整实例与事件所有权。
    /// @param source 来源会话；id 为工具所属插件
    /// @returns 不同版本发生冲突时返回错误
    pub(crate) fn inherit(&mut self, source: &Self, id: &str) -> Result<()> {
        let instance = source
            .instances
            .get(id)
            .with_context(|| format!("plugin instance missing: {id}"))?;
        if let Some(current) = self.instances.get(id) {
            if current.revision != instance.revision {
                bail!("plugin version conflict: {id}");
            }
        } else {
            self.insert(instance.clone());
        }
        Ok(())
    }

    /// 【插件】【命令目录】取得已启用插件注册的命令。
    /// @returns 按插件和命令名排序的命令定义
    pub(crate) fn commands(&self) -> Vec<(String, PluginCommand)> {
        self.instances
            .iter()
            .flat_map(|(id, instance)| {
                instance
                    .runtime
                    .commands()
                    .iter()
                    .map(|command| (id.clone(), command.clone()))
            })
            .collect()
    }

    /// 【插件】【工具执行】用宿主提供的真实上下文调用当前会话实例。
    /// @param id 插件 ID；name 为包内名称；args 为参数；context 为已授权上下文
    /// @returns 工具输出，不持有全局或父会话 VM
    pub(crate) async fn call_tool(
        &self,
        id: &str,
        name: &str,
        args: Value,
        context: InvocationContext,
    ) -> Result<String> {
        self.runtime(id)?.call_tool(name, args, context).await
    }

    /// 【插件】【命令执行】在同一插件实例中执行用户命令。
    /// @param id 插件 ID；name 为包内名称；args 为用户文本；context 为已授权上下文
    /// @returns 可直接显示的命令结果
    pub(crate) async fn call_command(
        &self,
        id: &str,
        name: &str,
        args: &str,
        context: InvocationContext,
    ) -> Result<String> {
        self.runtime(id)?.call_command(name, args, context).await
    }

    /// 【插件】【执行检查】工具检查事件只允许拒绝，不能修改参数或扩大宿主授权。
    /// @param name 实际执行工具名；args 为原始模型参数；context 为宿主上下文
    /// @returns 所有检查允许时成功；监听器异常也阻止本次工具执行
    pub(crate) async fn check_tool(
        &self,
        name: &str,
        args: &Value,
        context: &InvocationContext,
    ) -> Result<()> {
        for (id, instance) in self
            .instances
            .iter()
            .filter(|(_, instance)| instance.runtime.events().contains(&EventKind::ToolCall))
        {
            let results = instance
                .runtime
                .emit(
                    EventKind::ToolCall,
                    event_context(
                        context,
                        json!({
                            "name": name, "arguments": args,
                        }),
                    ),
                )
                .await?;
            for result in results {
                if result.is_null() {
                    continue;
                }
                let Some(fields) = result.as_object().filter(|fields| fields.len() == 1) else {
                    bail!("plugin {id} tool_call must return nil or {{deny = reason}}");
                };
                let reason = fields
                    .get("deny")
                    .and_then(Value::as_str)
                    .filter(|reason| !reason.trim().is_empty() && reason.len() <= 4096)
                    .with_context(|| format!("plugin {id} returned an invalid tool denial"))?;
                bail!("plugin {id} denied {name}: {reason}");
            }
        }
        Ok(())
    }

    /// 【插件】【事件通知】隔离观察型监听器错误，返回值不会隐式写入模型上下文。
    /// @param event 事件类型；context 为宿主上下文；data 为已确认的事件资料
    /// @returns 无；错误写入诊断，后续插件仍收到事件
    pub(crate) async fn notify(&self, event: EventKind, context: &InvocationContext, data: Value) {
        for (id, instance) in self
            .instances
            .iter()
            .filter(|(_, instance)| instance.runtime.events().contains(&event))
        {
            if let Err(error) = instance
                .runtime
                .emit(event, event_context(context, data.clone()))
                .await
            {
                eprintln!("【插件】【事件通知】{id} {event:?}: {error:#}");
            }
        }
    }

    /// 【插件】【实例查询】禁止禁用或未安装插件通过旧名称进入运行时。
    /// @param id 插件 ID
    /// @returns 当前集合中存在的运行实例
    fn runtime(&self, id: &str) -> Result<&PluginRuntime> {
        self.instances
            .get(id)
            .map(|instance| &instance.runtime)
            .with_context(|| format!("plugin is not active: {id}"))
    }
}

/// 【插件】【事件上下文】复制宿主会话字段，不信任工具参数中的身份信息。
/// @param context 宿主上下文；data 为本次事件数据
/// @returns 只读事件上下文
fn event_context(context: &InvocationContext, data: Value) -> EventContext {
    EventContext {
        session_id: context.session_id.clone(),
        workdir: context.workdir.clone(),
        data,
    }
}
