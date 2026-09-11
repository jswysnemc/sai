use super::{registration::json_value, Invocation, InvocationContext, PluginRuntime, Vm};
use crate::tool_policy::validate_state;
use crate::{ToolPolicyInput, ToolPolicyOutput, MAX_TOOL_POLICY_INPUT_BYTES};
use anyhow::{bail, Context, Result};
use mlua::{LuaSerdeExt, Table, Value as LuaValue};
use serde_json::Value;
use std::sync::atomic::Ordering;

impl PluginRuntime {
    /// 【工具策略】【注册查询】仅查询固定注册元数据，不取得虚拟机执行锁
    /// @returns 是否注册了可选工具后回调
    pub fn has_tool_policy(&self) -> bool {
        self.tool_policy_registered
    }

    /// 【工具策略】【受限入口】独立检查回复策略授权，禁止回调修改会话私有状态
    /// @param input 宿主工具事实；state 为当前循环状态；context 为可信调用归属
    /// @returns 校验后的下一轮状态与提醒；错误不返回部分结果
    pub async fn after_tool(
        &self,
        input: ToolPolicyInput,
        state: Value,
        mut context: InvocationContext,
    ) -> Result<ToolPolicyOutput> {
        if !self.tool_policy_registered || !self.reply_allowed {
            bail!("plugin tool policy is not allowed");
        }
        if context.session_id.is_empty()
            || context.operation_id.is_empty()
            || context.workdir.is_empty()
        {
            bail!("tool policy requires a session, operation and workdir");
        }
        validate_state(&state)?;
        let input = serde_json::to_value(input)?;
        if serde_json::to_vec(&input)?.len()
            > MAX_TOOL_POLICY_INPUT_BYTES.min(self.manifest.limits.output_bytes)
        {
            bail!("tool policy input exceeds size limit");
        }
        context.allow_writes = false;
        context.services = None;
        let value = self
            .invoke(Invocation::AfterTool(input, state), context)
            .await?;
        let output = if value.is_null() {
            ToolPolicyOutput::default()
        } else {
            serde_json::from_value::<ToolPolicyOutput>(value)
                .context("invalid tool policy result")?
        };
        output.validate()?;
        Ok(output)
    }
}

impl Vm {
    /// 【工具策略】【只读执行】沿用事件的私有状态只读权限，计数状态由宿主显式传递
    /// @param input 工具事实；state 为上次结果；ctx 为本次可信上下文
    /// @returns Lua 返回的完整 JSON 值
    pub(super) async fn after_tool(&self, input: Value, state: Value, ctx: Table) -> Result<Value> {
        let handler = self
            .reply_policy
            .as_ref()
            .and_then(|policy| policy.after_tool.as_ref())
            .context("plugin has no tool policy")?;
        self.control.writable.store(false, Ordering::Release);
        ctx.set("allow_writes", false)?;
        let value = handler
            .call_async::<LuaValue>((self.lua.to_value(&input)?, self.lua.to_value(&state)?, ctx))
            .await?;
        Ok(json_value(&self.lua, value)?)
    }
}
