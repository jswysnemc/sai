use super::registration::{ensure_open, json_value, lua_error, Registrations};
use super::Vm;
use anyhow::{Context, Result};
use mlua::{Function, Lua, Table, Value};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

/// 【回复策略】【阶段回调】每个插件只注册一组准备和完成函数
pub(super) struct RegisteredReplyPolicy {
    prepare: Function,
    complete: Function,
    pub(super) after_tool: Option<Function>,
}

/// 【回复策略】【注册入口】安装单一策略定义，拒绝重复、缺失回调及未知字段
/// @param lua 虚拟机；api 为公开接口；registrations 为加载事务
/// @returns 注册入口安装结果
pub(super) fn install(
    lua: &Lua,
    api: &Table,
    registrations: Arc<Mutex<Registrations>>,
) -> mlua::Result<()> {
    api.set(
        "register_reply_policy",
        lua.create_function(move |_, definition: Table| {
            let mut registrations = registrations.lock().map_err(lua_error)?;
            ensure_open(&registrations)?;
            if registrations.reply_policy.is_some() {
                return Err(mlua::Error::runtime("duplicate plugin reply policy"));
            }
            for field in definition.clone().pairs::<String, Value>() {
                let (name, _) = field?;
                if !matches!(name.as_str(), "prepare" | "complete" | "after_tool") {
                    return Err(mlua::Error::runtime("unknown reply policy field"));
                }
            }
            registrations.reply_policy = Some(RegisteredReplyPolicy {
                prepare: definition.get("prepare")?,
                complete: definition.get("complete")?,
                after_tool: definition.get("after_tool")?,
            });
            Ok(())
        })?,
    )
}

impl Vm {
    /// 【回复策略】【只读准备】准备可以读取授权数据和调用授权服务，但不能写入宿主
    /// @param input 用户输入；ctx 为当前 Lua 上下文；can_deliver 为宿主后续投递许可
    /// @returns 业务提供的准备资料
    pub(super) async fn prepare_reply(
        &self,
        input: String,
        ctx: Table,
        can_deliver: bool,
    ) -> Result<serde_json::Value> {
        let policy = self
            .reply_policy
            .as_ref()
            .context("plugin has no reply policy")?;
        self.control.writable.store(false, Ordering::Release);
        ctx.set("allow_writes", false)?;
        ctx.set("reply_can_deliver", can_deliver)?;
        let value = policy.prepare.call_async::<Value>((input, ctx)).await?;
        Ok(json_value(&self.lua, value)?)
    }

    /// 【回复策略】【完成执行】所属实例已验证准备结果，回调使用当前可信写入权限
    /// @param delivery 原准备资料；ctx 为 Lua 上下文；writable 为宿主权限
    /// @returns 业务更新后的上下文资料
    pub(super) async fn complete_reply(
        &self,
        delivery: serde_json::Value,
        ctx: Table,
        writable: bool,
    ) -> Result<serde_json::Value> {
        use mlua::LuaSerdeExt;
        let policy = self
            .reply_policy
            .as_ref()
            .context("plugin has no reply policy")?;
        self.control.writable.store(writable, Ordering::Release);
        ctx.set("allow_writes", writable)?;
        let value = policy
            .complete
            .call_async::<Value>((self.lua.to_value(&delivery)?, ctx))
            .await?;
        Ok(json_value(&self.lua, value)?)
    }
}
