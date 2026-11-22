use super::SuperAgentService;
use crate::super_agent::channel_bridge::{default_channel_target, ChannelInboundMessage};
use crate::super_agent::model::{GroupMessage, MessageKind, MessageSenderKind};
use crate::super_agent::policy::{evaluate_inbound_route, RoutingDecision};
use crate::super_agent::store::{NewGroupMessage, NewTriggerEvent};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::sync::Arc;

impl SuperAgentService {
    /// 持久化一条群组消息并推送实时事件。
    ///
    /// 参数:
    /// - `input`: 新消息参数
    ///
    /// 返回:
    /// - 已落库的消息记录
    pub fn record_group_message(&self, input: NewGroupMessage) -> Result<GroupMessage> {
        let message = self.store.append_message(input)?;
        self.emit_event(
            "message.created",
            Some(&message.group_id),
            json!({ "message": message }),
        );
        Ok(message)
    }

    /// 用户在 TUI 或 WebUI 群组内发送一条消息，并执行两级过滤/订阅直通/Leader 批处理路由。
    ///
    /// 参数:
    /// - `self`: 服务 Arc 句柄
    /// - `group_id`: 群组标识
    /// - `sender_name`: 用户名
    /// - `content`: 消息正文（支持 `@runner` / `@leader`）
    /// - `thread_id`: 可选指定 Thread
    /// - `recipient_id`: 可选显式目标成员 ID
    /// - `source_channel`: 来源渠道（tui / web / group）
    /// - `auto_dispatch`: 为 true 时立即唤醒目标 Leader 或 Runner
    ///
    /// 返回:
    /// - `(群组消息, 路由决策)`
    #[allow(clippy::too_many_arguments)]
    pub async fn post_user_message(
        self: &Arc<Self>,
        group_id: &str,
        sender_name: &str,
        content: &str,
        thread_id: Option<&str>,
        recipient_id: Option<&str>,
        source_channel: &str,
        auto_dispatch: bool,
    ) -> Result<(GroupMessage, RoutingDecision)> {
        let group = self
            .store
            .get_group(group_id)?
            .ok_or_else(|| anyhow!("group not found: {group_id}"))?;
        let active_thread_id = match thread_id {
            Some(tid) if !tid.trim().is_empty() => tid.trim().to_string(),
            _ => group.default_thread_id.clone(),
        };
        let members = self.store.list_members(group_id)?;
        let subs = self.store.thread_subscriptions(&active_thread_id)?;
        let decision = evaluate_inbound_route(content, recipient_id, &members, &subs, None);

        let (resolved_recipient_id, resolved_recipient_name) = match &decision {
            RoutingDecision::FastPathWorkers { worker_ids, .. } if worker_ids.len() == 1 => {
                let id = &worker_ids[0];
                let name = members
                    .iter()
                    .find(|m| &m.id == id)
                    .map(|m| m.name.clone());
                (Some(id.clone()), name)
            }
            RoutingDecision::SlowPathLeader { .. } => {
                let leader_name = members
                    .iter()
                    .find(|m| m.id == group.leader_member_id)
                    .map(|m| m.name.clone());
                (Some(group.leader_member_id.clone()), leader_name)
            }
            _ => (None, None),
        };

        // 1. 写入群组对话流
        let message = self.record_group_message(NewGroupMessage {
            group_id: group_id.to_string(),
            thread_id: active_thread_id.clone(),
            sender_kind: MessageSenderKind::User,
            sender_id: "user".to_string(),
            sender_name: if sender_name.trim().is_empty() {
                "You".to_string()
            } else {
                sender_name.trim().to_string()
            },
            recipient_id: resolved_recipient_id,
            recipient_name: resolved_recipient_name,
            kind: MessageKind::Chat,
            channel_source: Some(source_channel.to_string()),
            content: content.trim().to_string(),
            summary: None,
            related_task_id: None,
            related_report_id: None,
        })?;

        // 2. 记录 Trigger 审计去向并按 Fast Path / Slow Path 投递
        let targets = match &decision {
            RoutingDecision::FastPathWorkers { worker_ids, .. } => worker_ids.clone(),
            RoutingDecision::SlowPathLeader { .. } => vec![group.leader_member_id.clone()],
            RoutingDecision::Ignore { .. } => Vec::new(),
        };
        let reason = match &decision {
            RoutingDecision::Ignore { reason }
            | RoutingDecision::FastPathWorkers { reason, .. }
            | RoutingDecision::SlowPathLeader { reason } => reason.clone(),
        };
        let trigger = self.store.record_trigger(NewTriggerEvent {
            group_id: group_id.to_string(),
            thread_id: active_thread_id.clone(),
            source_channel: source_channel.to_string(),
            actor: message.sender_name.clone(),
            content: content.trim().to_string(),
            disposition: decision.initial_disposition(),
            route_path: decision.route_path().to_string(),
            reason,
            target_member_ids: targets,
            metadata: json!({ "message_id": message.id }),
        })?;
        self.emit_event(
            "trigger.recorded",
            Some(group_id),
            json!({ "trigger": trigger }),
        );

        if auto_dispatch {
            self.dispatch_decision(
                group_id,
                &active_thread_id,
                &message.sender_name,
                content,
                &decision,
            )
            .await?;
        }
        Ok((message, decision))
    }

    /// 【超级智能体】【多渠道入站】将外部渠道（微信/飞书/QQ/OneBot/模拟渠道）消息接入群组 Leader 或订阅直通 Runner。
    ///
    /// 参数:
    /// - `self`: 服务 Arc 句柄
    /// - `inbound`: 标准化渠道入站消息
    /// - `auto_dispatch`: 是否立即异步唤醒 Leader/Runner
    ///
    /// 返回:
    /// - `(群组 ID, 群组消息, 路由决策)`
    pub async fn ingest_channel_message(
        self: &Arc<Self>,
        inbound: ChannelInboundMessage,
        auto_dispatch: bool,
    ) -> Result<(String, GroupMessage, RoutingDecision)> {
        let channel = inbound.channel.trim().to_ascii_lowercase();
        let binding = self
            .store
            .resolve_channel_binding(&channel, &inbound.external_chat_id)?;
        let group = if let Some(explicit_gid) = inbound.group_id.as_deref() {
            self.store
                .get_group(explicit_gid)?
                .ok_or_else(|| anyhow!("group not found: {explicit_gid}"))?
        } else if let Some(ref matched) = binding {
            self.store
                .get_group(&matched.group_id)?
                .ok_or_else(|| anyhow!("bound group not found: {}", matched.group_id))?
        } else {
            self.ensure_default_group(None)?
        };
        if let Some(ref matched) = binding {
            let _ = self.store.touch_channel_binding(&matched.id, true);
        }

        // 1. 为外部渠道聊天建立或复用独立 Thread，保证跨轮连续性
        let thread_title = format!("{}:{}", channel, inbound.external_chat_id);
        let thread = self.store.ensure_channel_thread(
            &group.id,
            &channel,
            &inbound.external_chat_id,
            &thread_title,
        )?;
        let members = self.store.list_members(&group.id)?;
        let subs = self.store.thread_subscriptions(&thread.id)?;
        let decision = evaluate_inbound_route(
            &inbound.content,
            None,
            &members,
            &subs,
            binding.as_ref(),
        );

        let mut target_meta = inbound.reply_target.clone();
        if let Value::Object(map) = &mut target_meta {
            map.entry("external_chat_id")
                .or_insert_with(|| Value::String(inbound.external_chat_id.clone()));
            if inbound.simulate_outbound {
                map.insert("simulate".to_string(), Value::Bool(true));
            }
        } else {
            target_meta = default_channel_target(
                &inbound.external_chat_id,
                inbound.simulate_outbound,
            );
        }

        let message = self.record_group_message(NewGroupMessage {
            group_id: group.id.clone(),
            thread_id: thread.id.clone(),
            sender_kind: MessageSenderKind::User,
            sender_id: format!("channel:{channel}:{}", inbound.actor),
            sender_name: format!("{} [{}]", inbound.actor, channel),
            recipient_id: Some(group.leader_member_id.clone()),
            recipient_name: Some("Leader".to_string()),
            kind: MessageKind::ChannelIn,
            channel_source: Some(channel.clone()),
            content: inbound.content.trim().to_string(),
            summary: None,
            related_task_id: None,
            related_report_id: None,
        })?;

        let targets = match &decision {
            RoutingDecision::FastPathWorkers { worker_ids, .. } => worker_ids.clone(),
            RoutingDecision::SlowPathLeader { .. } => vec![group.leader_member_id.clone()],
            RoutingDecision::Ignore { .. } => Vec::new(),
        };
        let reason = match &decision {
            RoutingDecision::Ignore { reason }
            | RoutingDecision::FastPathWorkers { reason, .. }
            | RoutingDecision::SlowPathLeader { reason } => reason.clone(),
        };
        let trigger = self.store.record_trigger(NewTriggerEvent {
            group_id: group.id.clone(),
            thread_id: thread.id.clone(),
            source_channel: channel,
            actor: inbound.actor.clone(),
            content: inbound.content.trim().to_string(),
            disposition: decision.initial_disposition(),
            route_path: decision.route_path().to_string(),
            reason,
            target_member_ids: targets,
            metadata: target_meta,
        })?;
        self.emit_event(
            "trigger.recorded",
            Some(&group.id),
            json!({ "trigger": trigger }),
        );

        if auto_dispatch {
            self.dispatch_decision(
                &group.id,
                &thread.id,
                &inbound.actor,
                &inbound.content,
                &decision,
            )
            .await?;
        }
        Ok((group.id, message, decision))
    }

    /// 按路由决策实际唤醒目标 Runner 或 Leader。
    async fn dispatch_decision(
        self: &Arc<Self>,
        group_id: &str,
        _thread_id: &str,
        sender_name: &str,
        content: &str,
        decision: &RoutingDecision,
    ) -> Result<()> {
        match decision {
            RoutingDecision::Ignore { .. } => {}
            RoutingDecision::FastPathWorkers { worker_ids, .. } => {
                for worker_id in worker_ids {
                    self.deliver_to_runner_inbox(group_id, worker_id, sender_name, content)
                        .await?;
                }
            }
            RoutingDecision::SlowPathLeader { .. } => {
                self.leaders.wake(Arc::clone(self), group_id.to_string());
            }
        }
        Ok(())
    }
}
