use crate::InvocationContext;
use anyhow::{bail, Result};
use serde::Deserialize;
use serde_json::Value;

pub const MAX_REPLY_INPUT_BYTES: usize = 65_536;
pub const MAX_REPLY_CONTEXT_BYTES: usize = 16_384;
pub const MAX_REPLY_DELIVERY_BYTES: usize = 16_384;

/// 【回复策略】【准备结果】上下文可读取，投递资料只能由所属实例消费一次
pub struct PreparedReply {
    pub context: Option<String>,
    pub reminder: Option<String>,
    pub(crate) delivery: Option<Value>,
    pub(crate) instance_id: u64,
    pub(crate) binding: ReplyBinding,
    pub(crate) allow_writes: bool,
}

/// 【回复策略】【可信归属】准备结果绑定原会话、存储会话、用户操作及工作目录
#[derive(PartialEq, Eq)]
pub(crate) struct ReplyBinding {
    session: String,
    storage_session: String,
    operation: String,
    workdir: String,
}

impl ReplyBinding {
    /// 【回复策略】【固定归属】只从宿主上下文复制绑定字段
    /// @param context 当前可信调用上下文
    /// @returns 不包含 Lua 可变表或服务对象的绑定信息
    pub(crate) fn from_context(context: &InvocationContext) -> Self {
        Self {
            session: context.session_id.clone(),
            storage_session: context.storage_session_id.clone(),
            operation: context.operation_id.clone(),
            workdir: context.workdir.clone(),
        }
    }
}

impl PreparedReply {
    /// 【回复策略】【投递查询】检查当前准备结果是否包含后续动作
    /// @returns 是否存在尚未消费的投递资料
    pub fn has_delivery(&self) -> bool {
        self.delivery.is_some()
    }
}

/// 【回复策略】【准备协议】业务只提供上下文、当前轮提醒和不透明 JSON 对象
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct ReplyPreparation {
    pub context: Option<String>,
    pub reminder: Option<String>,
    pub delivery: Option<Value>,
}

impl ReplyPreparation {
    /// 【回复策略】【结果边界】限制模型上下文和延迟投递资料，拒绝只读调用安排写入
    /// @param allow_writes 宿主允许后续投递的真实权限
    /// @returns 数据形状、大小及阶段权限均合法时成功
    pub(crate) fn validate(&self, allow_writes: bool) -> Result<()> {
        validate_context(self.context.as_deref())?;
        validate_context(self.reminder.as_deref())?;
        if let Some(delivery) = &self.delivery {
            if !allow_writes {
                bail!("read-only reply preparation cannot schedule delivery");
            }
            if !delivery.is_object()
                || serde_json::to_vec(delivery)?.len() > MAX_REPLY_DELIVERY_BYTES
            {
                bail!("reply delivery must be an object within 16384 bytes");
            }
        }
        Ok(())
    }
}

/// 【回复策略】【完成协议】完成回调只更新该插件自己的上下文资源
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct ReplyCompletion {
    pub context: Option<String>,
}

/// 【回复策略】【上下文边界】限制单个插件提供的文字，允许常规换行与制表
/// @param text 可选上下文或当前轮提醒
/// @returns 长度和控制字符均合法时成功
pub(crate) fn validate_context(text: Option<&str>) -> Result<()> {
    if let Some(text) = text {
        if text.len() > MAX_REPLY_CONTEXT_BYTES
            || text
                .chars()
                .any(|ch| ch.is_control() && !matches!(ch, '\n' | '\r' | '\t'))
        {
            bail!("reply context exceeds 16384 bytes or contains control characters");
        }
        if text.contains("<context-resource") || text.contains("</context-resource") {
            bail!("reply context cannot contain host resource markers");
        }
    }
    Ok(())
}
