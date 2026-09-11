use super::{ToolProgress, ToolRegistry};
use anyhow::Result;
use sai_plugin_runtime::{InvocationContext, PreparedReply};
use std::{collections::BTreeMap, sync::atomic::Ordering, time::Duration};

const MAX_POLICIES: usize = 8;
const PHASE_TIMEOUT: Duration = Duration::from_secs(60);

/// 【回复策略】【上下文快照】每个插件的文字绑定产生它的实例，重载后不能沿用旧值
pub(crate) type PluginReplyContexts = BTreeMap<String, (u64, String)>;

/// 【回复策略】【轮次准备】只有当前轮持有不可克隆的投递资料，上下文与提醒可供投影读取
#[derive(Default)]
pub(crate) struct PreparedPluginReplies {
    pub contexts: PluginReplyContexts,
    pub reminder: Option<String>,
    pending: Vec<(String, u64, PreparedReply)>,
}

impl PreparedPluginReplies {
    /// 【回复策略】【投递查询】允许交互面在外部输出前结束正文渲染
    /// @returns 是否有当前轮待投递策略
    pub(crate) fn has_delivery(&self) -> bool {
        !self.pending.is_empty()
    }
}

impl ToolRegistry {
    /// 【回复策略】【当前标识】同步展示只用此目录过滤已经载入的上下文，不重新执行策略
    /// @returns 当前已启用且获授权的策略标识
    pub(crate) fn active_reply_policy_ids(&self) -> Vec<String> {
        self.plugins
            .reply_policies()
            .into_iter()
            .map(|(id, _)| id)
            .collect()
    }

    /// 【回复策略】【当前快照】只保留仍启用、获授权且实例一致的上下文
    /// @param contexts 原轮次或上轮快照
    /// @returns 插件标识到当前有效文字的映射
    pub(crate) fn current_reply_contexts(
        &self,
        contexts: &PluginReplyContexts,
    ) -> BTreeMap<String, String> {
        self.plugins
            .reply_policies()
            .into_iter()
            .filter_map(|(id, instance)| {
                contexts
                    .get(&id)
                    .filter(|(owner, _)| *owner == instance)
                    .map(|(_, text)| (id, text.clone()))
            })
            .collect()
    }

    /// 【回复策略】【只读准备】固定会话与模型来源，错误和整体期限不影响主模型回复
    /// @param input 当前用户消息；allow_writes 为当前 Agent 后续投递许可
    /// @returns 至多八个策略的上下文、提醒及延迟动作
    pub(crate) async fn prepare_plugin_replies(
        &self,
        input: &str,
        allow_writes: bool,
    ) -> PreparedPluginReplies {
        let deadline = tokio::time::Instant::now() + PHASE_TIMEOUT;
        let mut result = PreparedPluginReplies::default();
        let mut reminders = Vec::new();
        for (id, instance) in self.plugins.reply_policies().into_iter().take(MAX_POLICIES) {
            let context =
                self.plugin_context(ToolProgress::default(), self.reply_writable(allow_writes));
            match tokio::time::timeout_at(deadline, self.prepare_one_reply(&id, input, context))
                .await
            {
                Ok(Ok(prepared)) => {
                    if let Some(text) = &prepared.context {
                        result.contexts.insert(id.clone(), (instance, text.clone()));
                    }
                    if let Some(reminder) = &prepared.reminder {
                        reminders.push(reminder.clone());
                    }
                    if prepared.has_delivery() {
                        result.pending.push((id, instance, prepared));
                    }
                }
                Ok(Err(error)) => eprintln!("【回复策略】【准备失败】{id}: {error:#}"),
                Err(error) => {
                    eprintln!("【回复策略】【准备超时】{id}: {error}");
                    break;
                }
            }
        }
        if !reminders.is_empty() {
            result.reminder = Some(reminders.join("\n\n"));
        }
        result
    }

    /// 【回复策略】【模型服务】准备期只公开只读工具，空输入预览不绑定模型服务
    /// @param id 插件标识；input 为输入；context 保留原后续投递许可
    /// @returns 单实例的准备结果
    async fn prepare_one_reply(
        &self,
        id: &str,
        input: &str,
        mut context: InvocationContext,
    ) -> Result<PreparedReply> {
        let mut readonly = context.clone();
        readonly.allow_writes = false;
        let services = if input.is_empty() && !context.allow_writes {
            None
        } else {
            self.plugin_services(id, &readonly)?
        };
        if let Some(service) = &services {
            context.services = Some(service.clone());
        }
        let operation = self.plugins.prepare_reply(id, input, context);
        if let Some(service) = services {
            service
                .events()
                .agent_run(
                    serde_json::json!({"kind":"reply_prepare", "plugin_id":id}),
                    operation,
                )
                .await
        } else {
            operation.await
        }
    }

    /// 【回复策略】【完成分发】重新检查实例及计划模式，消费结果且不自动重试副作用
    /// @param prepared 本轮准备结果；allow_writes 为完成时最新 Agent 权限
    /// @returns 成功策略的上下文更新；失败策略不替换原快照
    pub(crate) async fn complete_plugin_replies(
        &self,
        prepared: PreparedPluginReplies,
        allow_writes: bool,
    ) -> BTreeMap<String, (u64, Option<String>)> {
        let mut updates = BTreeMap::new();
        let deadline = tokio::time::Instant::now() + PHASE_TIMEOUT;
        for (id, instance, plan) in prepared.pending {
            if !self.reply_writable(allow_writes)
                || !self
                    .plugins
                    .reply_policies()
                    .contains(&(id.clone(), instance))
            {
                continue;
            }
            let context = self.plugin_context(ToolProgress::default(), true);
            match tokio::time::timeout_at(deadline, self.complete_one_reply(&id, plan, context))
                .await
            {
                Ok(Ok(text)) => {
                    updates.insert(id, (instance, text));
                }
                Ok(Err(error)) => eprintln!("【回复策略】【完成失败】{id}: {error:#}"),
                Err(error) => {
                    eprintln!("【回复策略】【完成超时】{id}: {error}");
                    break;
                }
            }
        }
        updates
    }

    /// 【回复策略】【完成服务】使用当前模型和工具白名单，不沿用准备阶段服务句柄
    /// @param id 当前插件；plan 为原结果；context 为当前可信权限
    /// @returns 完成后的上下文
    async fn complete_one_reply(
        &self,
        id: &str,
        plan: PreparedReply,
        mut context: InvocationContext,
    ) -> Result<Option<String>> {
        let services = self.plugin_services(id, &context)?;
        if let Some(service) = &services {
            context.services = Some(service.clone());
        }
        let operation = self.plugins.complete_reply(id, plan, context);
        if let Some(service) = services {
            service
                .events()
                .agent_run(
                    serde_json::json!({"kind":"reply_complete", "plugin_id":id}),
                    operation,
                )
                .await
        } else {
            operation.await
        }
    }

    /// 【回复策略】【即时权限】调用方许可还需与注册表的当前计划模式共同检查
    /// @param allowed 调用方许可
    /// @returns 本次是否允许后续投递
    fn reply_writable(&self, allowed: bool) -> bool {
        allowed
            && self.permission_mode_handle().is_none_or(|mode| {
                crate::agent::AgentMode::from_u8(mode.load(Ordering::SeqCst))
                    != crate::agent::AgentMode::Plan
            })
    }
}
