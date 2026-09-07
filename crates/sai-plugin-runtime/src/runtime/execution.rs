use super::{registration, Invocation, InvocationContext, Vm};
use crate::ToolAccess;
use anyhow::{bail, Context, Result};
use mlua::{Lua, LuaSerdeExt, Table, Value as LuaValue};
use serde_json::Value;
use std::sync::atomic::Ordering;
use std::sync::Arc;

impl Vm {
    /// 【插件】【回调执行】根据注册契约验证输入并调用 Lua 函数。
    /// @param invocation 为工具、命令或事件；context 为宿主上下文
    /// @returns Lua 回调结果或契约校验错误
    pub(super) async fn execute(
        &mut self,
        invocation: Invocation,
        context: InvocationContext,
    ) -> Result<Value> {
        self.control.generation.fetch_add(1, Ordering::AcqRel);
        self.control.progress_messages.store(0, Ordering::Release);
        let ctx = context_table(&self.lua, &context, self.control.clone())?;
        self.control.active.store(true, Ordering::Release);
        match invocation {
            Invocation::Tool(name, arguments) => {
                let tool = self
                    .tools
                    .get(&name)
                    .with_context(|| format!("unknown plugin tool: {name}"))?;
                if !arguments.is_object() {
                    bail!("plugin tool arguments must be a JSON object");
                }
                tool.validator
                    .validate(&arguments)
                    .map_err(|error| anyhow::anyhow!("plugin tool {name} arguments: {error}"))?;
                self.control.writable.store(
                    context.allow_writes && tool.definition.access == ToolAccess::Writes,
                    Ordering::Release,
                );
                let value = tool
                    .handler
                    .call_async::<LuaValue>((self.lua.to_value(&arguments)?, ctx))
                    .await?;
                Ok(registration::json_value(&self.lua, value)?)
            }
            Invocation::Command(name, arguments) => {
                let command = self
                    .commands
                    .get(&name)
                    .with_context(|| format!("unknown plugin command: {name}"))?;
                self.control.writable.store(
                    context.allow_writes && command.definition.access == ToolAccess::Writes,
                    Ordering::Release,
                );
                let value = command
                    .handler
                    .call_async::<LuaValue>((arguments, ctx))
                    .await?;
                Ok(registration::json_value(&self.lua, value)?)
            }
            Invocation::Event(event, data) => {
                self.control.writable.store(false, Ordering::Release);
                let mut results = Vec::new();
                if let Some(handlers) = self.events.get(&event) {
                    for handler in handlers {
                        let value = handler
                            .call_async::<LuaValue>((self.lua.to_value(&data)?, ctx.clone()))
                            .await?;
                        results.push(registration::json_value(&self.lua, value)?);
                    }
                }
                Ok(Value::Array(results))
            }
        }
    }
}

/// 【插件】【上下文绑定】为单次调用构造会话字段和有界进度回调。
/// @param lua 为虚拟机；context 为可信宿主上下文；control 为单次调用的有效性和数量限制
/// @returns 供 Lua 读取的上下文表；修改该表不会改变宿主授权
fn context_table(
    lua: &Lua,
    context: &InvocationContext,
    control: Arc<super::control::CallControl>,
) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.set("session_id", context.session_id.as_str())?;
    table.set("workdir", context.workdir.as_str())?;
    let progress = context.progress.clone();
    let generation = control.generation.load(Ordering::Acquire);
    table.set(
        "progress",
        lua.create_function(move |_, text: String| {
            if !control.active.load(Ordering::Acquire)
                || generation != control.generation.load(Ordering::Acquire)
            {
                return Err(mlua::Error::runtime("plugin progress context has expired"));
            }
            if text.len() > 4096 || control.progress_messages.fetch_add(1, Ordering::Relaxed) >= 128
            {
                return Err(mlua::Error::runtime("plugin progress output exceeds limit"));
            }
            if let Some(progress) = &progress {
                progress(text);
            }
            Ok(())
        })?,
    )?;
    Ok(table)
}
