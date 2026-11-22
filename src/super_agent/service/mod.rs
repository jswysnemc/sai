mod delivery;
mod messaging;
mod runners;

use super::leader_runtime::LeaderCoordinator;
use super::model::{
    now_millis, Group, GroupDetailSnapshot, GroupMember, GroupStatus, MemberRole, MessageKind,
    MessageSenderKind, SuperAgentEvent, SuperAgentOverview, WatchdogEvent,
};
use super::store::{NewGroupMessage, SuperAgentStore};
use super::tools::{model_display_label, role_runtime_config};
use super::worker_executor::WorkerExecutor;
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use anyhow::Result;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;

/// 创建并启动 Runner 的请求结构。
#[derive(Debug, Clone)]
pub struct SpawnRunnerRequest {
    pub group_id: String,
    pub name: String,
    pub specialty: String,
    pub subagent_type: String,
    pub task_title: String,
    pub prompt: String,
    pub thread_id: Option<String>,
    pub origin_channel: Option<String>,
    pub origin_target: Option<Value>,
}

/// 【超级智能体】【统一门面服务】供 TUI、WebUI、多渠道网关与 CLI 共同调用。
pub struct SuperAgentService {
    pub(super) paths: SaiPaths,
    pub(super) store: SuperAgentStore,
    pub(super) workers: WorkerExecutor,
    pub(super) leaders: LeaderCoordinator,
    events_tx: broadcast::Sender<SuperAgentEvent>,
    event_seq: AtomicU64,
    watchdog_started: AtomicBool,
}

impl SuperAgentService {
    /// 打开存储并初始化 Super-Agent 服务。
    ///
    /// 参数:
    /// - `paths`: Sai 路径集合
    ///
    /// 返回:
    /// - 可共享的服务句柄
    pub fn open(paths: &SaiPaths) -> Result<Arc<Self>> {
        let store = SuperAgentStore::open(paths)?;
        let (events_tx, _) = broadcast::channel(512);
        Ok(Arc::new(Self {
            paths: paths.clone(),
            store,
            workers: WorkerExecutor::new(),
            leaders: LeaderCoordinator::new(),
            events_tx,
            event_seq: AtomicU64::new(0),
            watchdog_started: AtomicBool::new(false),
        }))
    }

    /// 返回 Sai 路径集合。
    pub fn paths(&self) -> &SaiPaths {
        &self.paths
    }

    /// 返回持久化存储引用。
    pub fn store(&self) -> &SuperAgentStore {
        &self.store
    }

    /// 返回 Worker 执行管理器引用。
    pub fn worker_executor(&self) -> &WorkerExecutor {
        &self.workers
    }

    /// 返回 Leader 批处理协调器引用。
    pub fn leader_coordinator(&self) -> &LeaderCoordinator {
        &self.leaders
    }

    /// 订阅实时增量事件流（供 WebUI SSE 与 TUI 实时刷新）。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - broadcast 接收端
    pub fn subscribe_events(&self) -> broadcast::Receiver<SuperAgentEvent> {
        self.events_tx.subscribe()
    }

    /// 确保后台 Watchdog 巡检回路已启动（幂等）。
    ///
    /// 参数:
    /// - `self`: 服务 Arc 句柄
    ///
    /// 返回:
    /// - 无
    pub fn ensure_watchdog_started(self: &Arc<Self>) {
        if !self.watchdog_started.swap(true, Ordering::AcqRel)
            && tokio::runtime::Handle::try_current().is_ok()
        {
            let _ = super::watchdog::spawn_watchdog_loop(Arc::clone(self));
        }
    }

    /// 广播一条 Super-Agent 实时事件。
    ///
    /// 参数:
    /// - `kind`: 事件类型名称
    /// - `group_id`: 关联群组标识
    /// - `payload`: 事件内容 JSON
    ///
    /// 返回:
    /// - 无
    pub fn emit_event(&self, kind: &str, group_id: Option<&str>, payload: Value) {
        let seq = self.event_seq.fetch_add(1, Ordering::Relaxed) + 1;
        let _ = self.events_tx.send(SuperAgentEvent {
            seq,
            kind: kind.to_string(),
            group_id: group_id.map(ToOwned::to_owned),
            payload,
            timestamp: now_millis(),
        });
    }

    /// 确保至少存在一个可用默认群组（首次进入 TUI/WebUI 或渠道首次接入时自动创建）。
    ///
    /// 参数:
    /// - `workspace_path`: 可选默认工作区路径
    ///
    /// 返回:
    /// - 默认群组记录
    pub fn ensure_default_group(&self, workspace_path: Option<&str>) -> Result<Group> {
        let config = AppConfig::load_or_default(&self.paths).unwrap_or_default();
        if let Some(default_id) = config.super_agent.default_group_id.as_deref() {
            if let Some(group) = self.store.get_group(default_id)? {
                return Ok(group);
            }
        }
        if let Some(first) = self.store.list_groups()?.into_iter().next() {
            return Ok(first);
        }
        let cwd = workspace_path
            .map(ToOwned::to_owned)
            .or_else(|| {
                crate::runtime_cwd::current_dir()
                    .ok()
                    .map(|path| path.display().to_string())
            })
            .unwrap_or_default();
        let (group, _) = self.create_group(
            "Super-Agent Workspace",
            "Leader + Runners collaborative group",
            &cwd,
            Some("Leader"),
        )?;
        Ok(group)
    }

    /// 创建新协作群组并写入欢迎系统消息。
    ///
    /// 参数:
    /// - `name`: 群组名称
    /// - `description`: 群组简介
    /// - `workspace_path`: 工作区路径
    /// - `leader_name`: 可选 Leader 名称
    ///
    /// 返回:
    /// - 新建群组与 Leader 成员
    pub fn create_group(
        &self,
        name: &str,
        description: &str,
        workspace_path: &str,
        leader_name: Option<&str>,
    ) -> Result<(Group, GroupMember)> {
        let config = AppConfig::load_or_default(&self.paths).unwrap_or_default();
        let leader_cfg = role_runtime_config(&config, true);
        let leader_model = model_display_label(&leader_cfg);
        let effective_ws = if workspace_path.trim().is_empty() {
            crate::runtime_cwd::current_dir()
                .ok()
                .map(|path| path.display().to_string())
                .unwrap_or_default()
        } else {
            workspace_path.trim().to_string()
        };
        let (group, leader) = self.store.create_group(
            name,
            description,
            &effective_ws,
            leader_name.unwrap_or("Leader"),
            &leader_model,
        )?;
        let _ = self.record_group_message(NewGroupMessage {
            group_id: group.id.clone(),
            thread_id: group.default_thread_id.clone(),
            sender_kind: MessageSenderKind::System,
            sender_id: "system".to_string(),
            sender_name: "System".to_string(),
            recipient_id: None,
            recipient_name: None,
            kind: MessageKind::Chat,
            channel_source: None,
            content: format!(
                "群组「{}」已就绪。Leader（{}）已上线；直接发送消息由 Leader 协调任务，或使用 @成员名 与指定 Runner 交流。",
                group.name, leader.name
            ),
            summary: None,
            related_task_id: None,
            related_report_id: None,
        });
        self.emit_event("group.created", Some(&group.id), json!({ "group": group }));
        Ok((group, leader))
    }

    /// 更新群组信息。
    pub fn update_group(
        &self,
        group_id: &str,
        name: Option<&str>,
        description: Option<&str>,
        status: Option<GroupStatus>,
    ) -> Result<Group> {
        let group = self
            .store
            .update_group(group_id, name, description, status)?;
        self.emit_event("group.updated", Some(group_id), json!({ "group": group }));
        Ok(group)
    }

    /// 删除群组并中止该群组内正在运行的全部 Runner。
    pub fn delete_group(&self, group_id: &str) -> Result<bool> {
        if let Ok(members) = self.store.list_members(group_id) {
            for member in members {
                if member.role == MemberRole::Worker {
                    self.workers.cancel_live(&member.id);
                }
            }
        }
        let deleted = self.store.delete_group(group_id)?;
        if deleted {
            self.emit_event(
                "group.deleted",
                Some(group_id),
                json!({ "group_id": group_id }),
            );
        }
        Ok(deleted)
    }

    /// 获取全局监控概览。
    pub fn overview(&self) -> Result<SuperAgentOverview> {
        self.store.overview()
    }

    /// 获取单个群组完整快照。
    pub fn group_detail(&self, group_id: &str, message_limit: usize) -> Result<GroupDetailSnapshot> {
        self.store.group_detail(group_id, message_limit)
    }

    /// 手动触发一次 Watchdog 健康巡检。
    pub async fn run_watchdog_check(self: &Arc<Self>) -> Result<Vec<WatchdogEvent>> {
        super::watchdog::run_watchdog_pass(self, true).await
    }
}
