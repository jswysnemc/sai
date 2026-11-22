use super::SuperAgentService;
use crate::super_agent::channel_bridge::deliver_to_channel;
use crate::super_agent::model::{
    ChannelBinding, ChannelDeliveryReceipt, GroupMessage, MessageKind, MessageSenderKind,
    ReportStatus, RunnerReport, TaskStatus, TriggerDisposition,
};
use crate::super_agent::store::{NewGroupMessage, NewRunnerReport, NewTriggerEvent};
use anyhow::{anyhow, Result};
use serde_json::json;
use std::sync::Arc;

impl SuperAgentService {
    /// 【超级智能体】【成果上报】Runner 提交结构化工作报告给 Leader，必要时自动唤醒 Leader 归纳交付。
    #[allow(clippy::too_many_arguments)]
    pub async fn submit_runner_report(
        self: &Arc<Self>,
        group_id: &str,
        thread_id: &str,
        task_id: Option<&str>,
        worker_member_id: &str,
        worker_name: &str,
        status: ReportStatus,
        summary: &str,
        details: &str,
        artifacts: Vec<String>,
        needs_human_question: Option<String>,
    ) -> Result<RunnerReport> {
        let group = self
            .store
            .get_group(group_id)?
            .ok_or_else(|| anyhow!("group not found: {group_id}"))?;
        let report = self.store.create_report(NewRunnerReport {
            group_id: group_id.to_string(),
            thread_id: thread_id.to_string(),
            task_id: task_id.map(ToOwned::to_owned),
            worker_member_id: worker_member_id.to_string(),
            worker_name: worker_name.to_string(),
            status,
            summary: summary.to_string(),
            details: details.to_string(),
            artifacts,
            needs_human_question,
        })?;
        // 1. 在群聊流发布结构化报告卡片消息（展示给用户与 Leader）
        let leader_name = self
            .store
            .get_member(&group.leader_member_id)?
            .map(|m| m.name)
            .unwrap_or_else(|| "Leader".to_string());
        let body = if details.trim().is_empty() || details.trim() == summary.trim() {
            format!("[Report: {}] {}", status.as_str(), summary.trim())
        } else {
            format!(
                "[Report: {}] {}\n\n{}",
                status.as_str(),
                summary.trim(),
                details.trim()
            )
        };
        let _ = self.record_group_message(NewGroupMessage {
            group_id: group_id.to_string(),
            thread_id: thread_id.to_string(),
            sender_kind: MessageSenderKind::Worker,
            sender_id: worker_member_id.to_string(),
            sender_name: worker_name.to_string(),
            recipient_id: Some(group.leader_member_id.clone()),
            recipient_name: Some(leader_name),
            kind: MessageKind::Report,
            channel_source: None,
            content: body,
            summary: Some(summary.trim().to_string()),
            related_task_id: task_id.map(ToOwned::to_owned),
            related_report_id: Some(report.id.clone()),
        });
        self.emit_event("report.submitted", Some(group_id), json!({ "report": report }));

        // 2. 若报告要求 Leader 归纳交付，自动入队 Trigger 并唤醒 Leader 批处理
        if status.requires_leader_synthesis() {
            let _ = self.store.record_trigger(NewTriggerEvent {
                group_id: group_id.to_string(),
                thread_id: thread_id.to_string(),
                source_channel: "worker_report".to_string(),
                actor: worker_name.to_string(),
                content: format!(
                    "Runner {} submitted {} report ({}): {}",
                    worker_name,
                    status.as_str(),
                    report.id,
                    summary.trim()
                ),
                disposition: TriggerDisposition::Queued,
                route_path: "slow_path".to_string(),
                reason: "runner_report_for_leader_synthesis".to_string(),
                target_member_ids: vec![group.leader_member_id.clone()],
                metadata: json!({
                    "report_id": report.id,
                    "task_id": report.task_id,
                    "worker_id": worker_member_id,
                }),
            });
            if tokio::runtime::Handle::try_current().is_ok() {
                self.leaders.wake(Arc::clone(self), group_id.to_string());
            }
        }
        Ok(report)
    }

    /// 【超级智能体】【主动归纳交付】Leader 将综合提炼后的成果正式发布给用户，并推送回来源渠道。
    #[allow(clippy::too_many_arguments)]
    pub async fn deliver_leader_result(
        &self,
        group_id: &str,
        thread_id: &str,
        task_id: Option<&str>,
        report_ids: &[String],
        summary: &str,
        deliverable: &str,
        channel_override: Option<&str>,
    ) -> Result<(GroupMessage, ChannelDeliveryReceipt)> {
        let group = self
            .store
            .get_group(group_id)?
            .ok_or_else(|| anyhow!("group not found: {group_id}"))?;
        let leader_name = self
            .store
            .get_member(&group.leader_member_id)?
            .map(|m| m.name)
            .unwrap_or_else(|| "Leader".to_string());

        // 1. 解析任务与目标回推渠道
        let task = match task_id {
            Some(tid) => self.store.get_task(tid)?,
            None => None,
        };
        let target_channel = channel_override
            .map(ToOwned::to_owned)
            .or_else(|| task.as_ref().map(|t| t.origin_channel.clone()))
            .unwrap_or_else(|| "group".to_string());
        let target_json = task
            .as_ref()
            .map(|t| t.origin_target.clone())
            .unwrap_or_else(|| json!({}));

        // 2. 更新任务为已交付，标记关联报告为已归纳
        if let Some(tid) = task_id {
            let _ = self.store.update_task(
                tid,
                Some(TaskStatus::Delivered),
                None,
                None,
                Some(summary.trim().to_string()),
                Some(deliverable.trim().to_string()),
            );
        }
        let _ = self.store.mark_reports_synthesized(report_ids, task_id)?;

        // 3. 在群组中写入正式交付消息
        let message = self.record_group_message(NewGroupMessage {
            group_id: group_id.to_string(),
            thread_id: thread_id.to_string(),
            sender_kind: MessageSenderKind::Leader,
            sender_id: group.leader_member_id.clone(),
            sender_name: leader_name,
            recipient_id: Some("user".to_string()),
            recipient_name: Some("User".to_string()),
            kind: MessageKind::Delivery,
            channel_source: Some(target_channel.clone()),
            content: deliverable.trim().to_string(),
            summary: Some(summary.trim().to_string()),
            related_task_id: task_id.map(ToOwned::to_owned),
            related_report_id: report_ids.first().cloned(),
        })?;

        // 4. 主动推送回来源外部渠道（微信/飞书/QQ/OneBot）并记录回执
        let outbound_text = if matches!(
            target_channel.as_str(),
            "weixin" | "wechat" | "feishu" | "qq" | "onebot"
        ) {
            summary.trim()
        } else {
            deliverable.trim()
        };
        let receipt = deliver_to_channel(
            &self.paths,
            &self.store,
            group_id,
            task_id,
            &target_channel,
            &target_json,
            outbound_text,
        )
        .await?;
        let _ = self
            .store
            .resolve_watchdog_kind(Some(group_id), "undelivered_report_nudge");
        self.emit_event(
            "delivery.completed",
            Some(group_id),
            json!({ "message": message, "receipt": receipt }),
        );
        Ok((message, receipt))
    }

    /// 绑定外部渠道到群组。
    pub fn bind_channel(
        &self,
        group_id: &str,
        channel: &str,
        external_chat_id: &str,
        label: &str,
        admission_mode: &str,
        enabled: bool,
    ) -> Result<ChannelBinding> {
        let binding = self.store.upsert_channel_binding(
            group_id,
            channel,
            external_chat_id,
            label,
            admission_mode,
            enabled,
        )?;
        self.emit_event(
            "channel.bound",
            Some(group_id),
            json!({ "binding": binding }),
        );
        Ok(binding)
    }

    /// 解绑外部渠道。
    pub fn unbind_channel(&self, group_id: &str, binding_id: &str) -> Result<bool> {
        let removed = self.store.delete_channel_binding(binding_id)?;
        if removed {
            self.emit_event(
                "channel.unbound",
                Some(group_id),
                json!({ "binding_id": binding_id }),
            );
        }
        Ok(removed)
    }
}
