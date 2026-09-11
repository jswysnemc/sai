use super::{Invocation, InvocationContext, PluginRuntime};
use crate::reply_policy::{validate_context, ReplyBinding, ReplyCompletion, ReplyPreparation};
use crate::{PreparedReply, MAX_REPLY_INPUT_BYTES};
use anyhow::{bail, Context, Result};

impl PluginRuntime {
    /// 【回复策略】【注册查询】只查询加载时固定的策略元数据
    /// @returns 是否存在回复策略，不代表已经取得执行授权
    pub fn has_reply_policy(&self) -> bool {
        self.reply_registered
    }

    /// 【回复策略】【准备入口】检查独立授权，并把返回资料绑定到原实例和可信上下文
    /// @param input 当前用户消息；context 为宿主会话、目录和后续投递许可
    /// @returns 上下文、当前轮提醒及不可克隆的准备结果
    pub async fn prepare_reply(
        &self,
        input: &str,
        context: InvocationContext,
    ) -> Result<PreparedReply> {
        self.check_reply_policy()?;
        if context.session_id.is_empty()
            || context.operation_id.is_empty()
            || context.workdir.is_empty()
        {
            bail!("reply policy requires a session, operation and workdir");
        }
        if input.len() > MAX_REPLY_INPUT_BYTES.min(self.manifest.limits.output_bytes) {
            bail!("reply policy input exceeds size limit");
        }
        let binding = ReplyBinding::from_context(&context);
        let allow_writes = context.allow_writes;
        let value = self
            .invoke(Invocation::ReplyPrepare(input.to_string()), context)
            .await?;
        let output = if value.is_null() {
            ReplyPreparation::default()
        } else {
            serde_json::from_value::<ReplyPreparation>(value)
                .context("invalid reply preparation")?
        };
        output.validate(allow_writes)?;
        Ok(PreparedReply {
            context: output.context,
            reminder: output.reminder,
            delivery: output.delivery,
            instance_id: self.instance_id,
            binding,
            allow_writes,
        })
    }

    /// 【回复策略】【单次投递】消费原准备结果，拒绝跨实例、跨操作、换目录及权限升级
    /// @param prepared 原实例返回的准备结果；context 为当前可信调用上下文
    /// @returns 完成后的插件上下文；回调错误不会重试或复用已消费资料
    pub async fn complete_reply(
        &self,
        prepared: PreparedReply,
        context: InvocationContext,
    ) -> Result<Option<String>> {
        self.check_reply_policy()?;
        if prepared.instance_id != self.instance_id
            || prepared.binding != ReplyBinding::from_context(&context)
        {
            bail!("reply preparation belongs to another instance or invocation");
        }
        if !prepared.allow_writes || !context.allow_writes {
            bail!("read-only invocation cannot complete reply delivery");
        }
        let Some(delivery) = prepared.delivery else {
            return Ok(prepared.context);
        };
        let value = self
            .invoke(Invocation::ReplyComplete(delivery), context)
            .await?;
        let output = if value.is_null() {
            ReplyCompletion::default()
        } else {
            serde_json::from_value::<ReplyCompletion>(value).context("invalid reply completion")?
        };
        validate_context(output.context.as_deref())?;
        Ok(output.context)
    }

    /// 【回复策略】【能力检查】策略注册和独立声明授权缺一不可
    /// @returns 当前实例可以执行策略时成功
    fn check_reply_policy(&self) -> Result<()> {
        if !self.reply_registered || !self.reply_allowed {
            bail!("plugin reply policy is not allowed");
        }
        Ok(())
    }
}
