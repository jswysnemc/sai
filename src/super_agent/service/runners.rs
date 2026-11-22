use super::{SpawnRunnerRequest, SuperAgentService};
use crate::config::AppConfig;
use crate::super_agent::model::{
    GroupMember, MemberStatus, MessageKind, MessageSenderKind, SubscriptionMode, SuperTask,
    TaskStatus, ThreadSubscription,
};
use crate::super_agent::store::{NewGroupMessage, NewSuperTask};
use crate::super_agent::tools::{model_display_label, role_runtime_config};
use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};
use std::sync::Arc;

impl SuperAgentService {
    /// 创建新 Worker(Runner)、建立任务工单、订阅对应 Thread 并异步启动执行。
    ///
    /// 参数:
    /// - `self`: 服务 Arc 句柄
    /// - `req`: Runner 创建与任务载荷
    ///
    /// 返回:
    /// - `(新建 Worker 成员, 关联任务工单)`
    pub async fn spawn_runner(
        self: &Arc<Self>,
        req: SpawnRunnerRequest,
    ) -> Result<(GroupMember, SuperTask)> {
        let group = self
            .store
            .get_group(&req.group_id)?
            .ok_or_else(|| anyhow!("group not found: {}", req.group_id))?;
        let config = AppConfig::load_or_default(&self.paths).unwrap_or_default();
        let running = self
            .store
            .list_running_workers()?
            .into_iter()
            .filter(|w| self.workers.is_live(&w.id))
            .count();
        if running >= config.super_agent.max_concurrent_workers {
            bail!(
                "max concurrent runners reached ({}/{}); wait for a runner to finish or stop one",
                running,
                config.super_agent.max_concurrent_workers
            );
        }
        let worker_cfg = role_runtime_config(&config, false);
        let worker_model = model_display_label(&worker_cfg);
        let thread_id = req
            .thread_id
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| group.default_thread_id.clone());
        // 1. 创建 Worker 成员与任务工单
        let worker = self.store.create_worker_member(
            &req.group_id,
            &req.name,
            &req.specialty,
            &req.subagent_type,
            &worker_model,
            &group.workspace_path,
            None,
        )?;
        let task = self.store.create_task(NewSuperTask {
            group_id: req.group_id.clone(),
            thread_id: thread_id.clone(),
            title: req.task_title.clone(),
            objective: req.prompt.clone(),
            assigned_member_id: Some(worker.id.clone()),
            assigned_member_name: Some(worker.name.clone()),
            origin_channel: req.origin_channel.unwrap_or_else(|| "group".to_string()),
            origin_target: req.origin_target.unwrap_or_else(|| json!({})),
        })?;
        // 2. 自动为该 Runner 订阅所属 Thread（开启 Fast-Path 直通）
        let _ = self.store.upsert_subscription(
            &req.group_id,
            &thread_id,
            &worker.id,
            SubscriptionMode::Owner,
        )?;
        // 3. 在群聊记录 Leader 派工动态
        let leader_name = self
            .store
            .get_member(&group.leader_member_id)?
            .map(|m| m.name)
            .unwrap_or_else(|| "Leader".to_string());
        let _ = self.record_group_message(NewGroupMessage {
            group_id: req.group_id.clone(),
            thread_id: thread_id.clone(),
            sender_kind: MessageSenderKind::Leader,
            sender_id: group.leader_member_id.clone(),
            sender_name: leader_name,
            recipient_id: Some(worker.id.clone()),
            recipient_name: Some(worker.name.clone()),
            kind: MessageKind::Dispatch,
            channel_source: None,
            content: format!(
                "已派发任务「{}」给 @{}：\n{}",
                task.title, worker.name, task.objective
            ),
            summary: Some(task.title.clone()),
            related_task_id: Some(task.id.clone()),
            related_report_id: None,
        });
        // 4. 异步启动 Worker 执行循环（仅在 Tokio 运行时中执行）
        if tokio::runtime::Handle::try_current().is_ok() {
            self.workers.launch(
                Arc::clone(self),
                req.group_id.clone(),
                worker.id.clone(),
                thread_id,
                Some(task.id.clone()),
                req.prompt,
            );
        }
        self.emit_event(
            "runner.created",
            Some(&req.group_id),
            json!({ "member": worker, "task": task }),
        );
        Ok((worker, task))
    }

    /// 为已有存活 Runner 派发新任务。
    #[allow(clippy::too_many_arguments)]
    pub async fn dispatch_task_to_runner(
        self: &Arc<Self>,
        group_id: &str,
        member_id: &str,
        title: &str,
        objective: &str,
        thread_id: &str,
        origin_channel: &str,
        origin_target: Value,
    ) -> Result<SuperTask> {
        let member = self
            .store
            .get_member(member_id)?
            .ok_or_else(|| anyhow!("runner not found: {member_id}"))?;
        let task = self.store.create_task(NewSuperTask {
            group_id: group_id.to_string(),
            thread_id: thread_id.to_string(),
            title: title.to_string(),
            objective: objective.to_string(),
            assigned_member_id: Some(member.id.clone()),
            assigned_member_name: Some(member.name.clone()),
            origin_channel: origin_channel.to_string(),
            origin_target,
        })?;
        let _ =
            self.store
                .upsert_subscription(group_id, thread_id, &member.id, SubscriptionMode::Owner)?;
        let prompt = format!("新分配任务「{title}」（task_id={}）：\n{objective}", task.id);
        if !self.workers.enqueue_live_message(member_id, "Leader", &prompt)
            && tokio::runtime::Handle::try_current().is_ok()
        {
            self.workers.launch(
                Arc::clone(self),
                group_id.to_string(),
                member.id.clone(),
                thread_id.to_string(),
                Some(task.id.clone()),
                prompt,
            );
        }
        self.emit_event("task.dispatched", Some(group_id), json!({ "task": task }));
        Ok(task)
    }

    /// 向 Runner 发送追问消息并记录到群组时间线。
    pub async fn message_runner(
        self: &Arc<Self>,
        group_id: &str,
        member_id: &str,
        sender_name: &str,
        message: &str,
    ) -> Result<()> {
        let group = self
            .store
            .get_group(group_id)?
            .ok_or_else(|| anyhow!("group not found: {group_id}"))?;
        let member = self
            .store
            .get_member(member_id)?
            .ok_or_else(|| anyhow!("runner not found: {member_id}"))?;
        let _ = self.record_group_message(NewGroupMessage {
            group_id: group_id.to_string(),
            thread_id: group.default_thread_id.clone(),
            sender_kind: if sender_name == "You" || sender_name == "user" {
                MessageSenderKind::User
            } else {
                MessageSenderKind::Leader
            },
            sender_id: sender_name.to_string(),
            sender_name: sender_name.to_string(),
            recipient_id: Some(member.id.clone()),
            recipient_name: Some(member.name.clone()),
            kind: MessageKind::Chat,
            channel_source: None,
            content: format!("@{} {}", member.name, message.trim()),
            summary: None,
            related_task_id: member.active_task_id.clone(),
            related_report_id: None,
        });
        self.deliver_to_runner_inbox(group_id, member_id, sender_name, message)
            .await
    }

    /// 将消息直接投入运行中 Runner 的步间信箱；若 Runner 处于空闲则重新拉起执行。
    pub async fn deliver_to_runner_inbox(
        self: &Arc<Self>,
        group_id: &str,
        member_id: &str,
        sender_name: &str,
        text: &str,
    ) -> Result<()> {
        if self.workers.enqueue_live_message(member_id, sender_name, text) {
            self.emit_event(
                "runner.messaged",
                Some(group_id),
                json!({ "member_id": member_id, "from": sender_name }),
            );
            return Ok(());
        }
        let member = self
            .store
            .get_member(member_id)?
            .ok_or_else(|| anyhow!("runner not found: {member_id}"))?;
        let group = self
            .store
            .get_group(group_id)?
            .ok_or_else(|| anyhow!("group not found: {group_id}"))?;
        if tokio::runtime::Handle::try_current().is_ok() {
            let prompt = format!("收到来自 {sender_name} 的群组消息：\n{text}");
            self.workers.launch(
                Arc::clone(self),
                group_id.to_string(),
                member.id,
                group.default_thread_id,
                member.active_task_id,
                prompt,
            );
        }
        Ok(())
    }

    /// 停止指定 Runner。
    pub fn stop_runner(&self, group_id: &str, member_id: &str, reason: &str) -> Result<GroupMember> {
        self.workers.cancel_live(member_id);
        let updated = self.store.update_member_state(
            member_id,
            MemberStatus::Stopped,
            Some(reason),
            Some(None),
        )?;
        self.emit_event(
            "runner.stopped",
            Some(group_id),
            json!({ "member": updated, "reason": reason }),
        );
        Ok(updated)
    }

    /// 绑定或更新 Thread 订阅关系。
    pub fn subscribe_thread(
        &self,
        group_id: &str,
        thread_id: &str,
        member_id: &str,
        mode: SubscriptionMode,
    ) -> Result<ThreadSubscription> {
        let sub = self
            .store
            .upsert_subscription(group_id, thread_id, member_id, mode)?;
        self.emit_event(
            "subscription.updated",
            Some(group_id),
            json!({ "subscription": sub }),
        );
        Ok(sub)
    }

    /// 取消 Thread 订阅关系。
    pub fn unsubscribe_thread(
        &self,
        group_id: &str,
        thread_id: &str,
        member_id: &str,
    ) -> Result<bool> {
        let removed = self.store.remove_subscription(thread_id, member_id)?;
        if removed {
            self.emit_event(
                "subscription.removed",
                Some(group_id),
                json!({ "thread_id": thread_id, "member_id": member_id }),
            );
        }
        Ok(removed)
    }

    /// 重试失败或受阻任务。
    pub async fn retry_task(self: &Arc<Self>, group_id: &str, task_id: &str) -> Result<SuperTask> {
        let task = self
            .store
            .get_task(task_id)?
            .ok_or_else(|| anyhow!("task not found: {task_id}"))?;
        let updated = self.store.update_task(
            task_id,
            Some(TaskStatus::Running),
            None,
            None,
            None,
            None,
        )?;
        if let Some(member_id) = task.assigned_member_id.as_deref() {
            let prompt = format!(
                "请重试并完成任务「{}」（task_id={}）：\n{}",
                task.title, task.id, task.objective
            );
            if tokio::runtime::Handle::try_current().is_ok() {
                self.workers.launch(
                    Arc::clone(self),
                    group_id.to_string(),
                    member_id.to_string(),
                    task.thread_id,
                    Some(task.id),
                    prompt,
                );
            }
        }
        self.emit_event("task.retried", Some(group_id), json!({ "task": updated }));
        Ok(updated)
    }

    /// 停止指定任务及其执行 Runner。
    pub fn stop_task(&self, group_id: &str, task_id: &str) -> Result<SuperTask> {
        let task = self
            .store
            .get_task(task_id)?
            .ok_or_else(|| anyhow!("task not found: {task_id}"))?;
        if let Some(member_id) = task.assigned_member_id.as_deref() {
            let _ = self.stop_runner(group_id, member_id, "task stopped");
        }
        let updated = self.store.update_task(
            task_id,
            Some(TaskStatus::Stopped),
            None,
            None,
            Some("stopped by user".to_string()),
            None,
        )?;
        self.emit_event("task.stopped", Some(group_id), json!({ "task": updated }));
        Ok(updated)
    }
}
