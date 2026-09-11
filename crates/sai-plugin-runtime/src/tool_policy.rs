use crate::reply_policy::validate_context;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MAX_TOOL_POLICY_INPUT_BYTES: usize = 65_536;
pub const MAX_TOOL_POLICY_STATE_BYTES: usize = 16_384;

/// 【工具策略】【调用事实】宿主提供实际工具结果与当前可见的本插件工具名称
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ToolPolicyInput {
    pub name: String,
    #[serde(default)]
    pub local_name: Option<String>,
    pub arguments: Value,
    pub ok: bool,
    pub tools: Vec<String>,
}

/// 【工具策略】【回调结果】状态仅供当前工具循环下次调用使用，提醒只追加当前请求
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ToolPolicyOutput {
    pub state: Value,
    pub reminder: Option<String>,
}

impl ToolPolicyOutput {
    /// 【工具策略】【结果校验】沿用回复文本边界，状态必须为有界 JSON 对象或 null
    /// @returns 可以进入当前轮状态与模型上下文时成功
    pub(crate) fn validate(&self) -> Result<()> {
        validate_state(&self.state)?;
        validate_context(self.reminder.as_deref())
    }
}

/// 【工具策略】【状态边界】拒绝数组、标量与超限状态，防止跨工具轮累积无界数据
/// @param state 上次或本次回调状态
/// @returns 状态形状和编码大小合法时成功
pub(crate) fn validate_state(state: &Value) -> Result<()> {
    if (!state.is_null() && !state.is_object())
        || serde_json::to_vec(state)?.len() > MAX_TOOL_POLICY_STATE_BYTES
    {
        bail!("tool policy state must be an object within 16384 bytes");
    }
    Ok(())
}
