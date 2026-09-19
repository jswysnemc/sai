use super::registration::{ensure_open, json_value, lua_error, Registrations};
use super::{Invocation, InvocationContext, PluginRuntime, Vm};
use crate::{
    PermissionAuditDecision, PermissionAuditInput, PermissionAuditOutput,
    MAX_PERMISSION_AUDIT_INPUT_BYTES,
};
use anyhow::{bail, Context, Result};
use mlua::{Function, Lua, LuaSerdeExt, Table, Value};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

/// 【权限审核】【注册入口】每个插件只允许注册一个审核函数
/// 参数: lua 为虚拟机，api 为公开接口，registrations 为加载事务；返回安装结果
pub(super) fn install(
    lua: &Lua,
    api: &Table,
    registrations: Arc<Mutex<Registrations>>,
) -> mlua::Result<()> {
    api.set(
        "register_permission_audit",
        lua.create_function(move |_, definition: Table| {
            let mut registrations = registrations.lock().map_err(lua_error)?;
            ensure_open(&registrations)?;
            if registrations.permission_audit.is_some() {
                return Err(mlua::Error::runtime("duplicate permission audit callback"));
            }
            for field in definition.clone().pairs::<String, Value>() {
                if field?.0 != "review" {
                    return Err(mlua::Error::runtime("unknown permission audit field"));
                }
            }
            registrations.permission_audit = Some(definition.get::<Function>("review")?);
            Ok(())
        })?,
    )
}

impl PluginRuntime {
    /// 返回是否注册审核回调，无参数
    pub fn has_permission_audit(&self) -> bool {
        self.audit_registered
    }

    /// 【权限审核】【受限执行】审核不能执行工具、修改状态或进行未授权网络写入
    /// 参数: input 为完整审核事实，context 为可信归属；返回校验后的决定
    pub async fn review_permission(
        &self,
        input: PermissionAuditInput,
        mut context: InvocationContext,
    ) -> Result<PermissionAuditOutput> {
        if !self.audit_registered || !self.audit_allowed {
            bail!("plugin permission audit is not allowed");
        }
        if context.session_id.is_empty()
            || context.operation_id.is_empty()
            || context.workdir.is_empty()
        {
            bail!("permission audit requires a session, operation and workdir");
        }
        if input.tool.trim().is_empty()
            || !input.arguments.is_object()
            || input.policy.trim().is_empty()
        {
            bail!("permission audit requires a tool, object arguments and policy");
        }
        let arguments_json = serde_json::to_string(&input.arguments)?;
        let mut input = serde_json::to_value(input)?;
        if serde_json::to_vec(&input)?.len() > MAX_PERMISSION_AUDIT_INPUT_BYTES {
            bail!("permission audit input exceeds size limit");
        }
        // 1. 【权限审核】【参数保真】保留 JSON 文本，避免 Lua 数值转换舍入超大整数
        input["arguments_json"] = serde_json::Value::String(arguments_json);
        context.allow_writes = false;
        context.services = None;
        context.progress = None;
        let value = self
            .invoke(Invocation::PermissionAudit(input), context)
            .await?;
        let output = if value.is_null() {
            PermissionAuditOutput {
                decision: PermissionAuditDecision::Abstain,
                reason: None,
            }
        } else {
            serde_json::from_value::<PermissionAuditOutput>(value)
                .context("invalid permission audit result")?
        };
        output.validate()?;
        Ok(output)
    }
}

impl Vm {
    /// 【权限审核】【只读回调】将工具事实交给 Lua，保持宿主写入开关关闭
    /// 参数: input 为 JSON 事实，ctx 为调用上下文；返回 Lua 结果
    pub(super) async fn review_permission(
        &self,
        input: serde_json::Value,
        ctx: Table,
    ) -> Result<serde_json::Value> {
        let handler = self
            .permission_audit
            .as_ref()
            .context("plugin has no permission audit callback")?;
        self.control.writable.store(false, Ordering::Release);
        ctx.set("allow_writes", false)?;
        let value = handler
            .call_async::<Value>((self.lua.to_value(&input)?, ctx))
            .await?;
        Ok(json_value(&self.lua, value)?)
    }
}
