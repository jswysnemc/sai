use super::SuperAgentStore;
use crate::super_agent::model::{
    new_id, now_millis, ChannelBinding, ChannelDeliveryReceipt, TriggerDisposition, TriggerEvent,
};
use anyhow::Result;
use rusqlite::{params, OptionalExtension};
use serde_json::Value;

/// 创建新 Trigger 的输入载荷。
#[derive(Debug, Clone)]
pub struct NewTriggerEvent {
    pub group_id: String,
    pub thread_id: String,
    pub source_channel: String,
    pub actor: String,
    pub content: String,
    pub disposition: TriggerDisposition,
    pub route_path: String,
    pub reason: String,
    pub target_member_ids: Vec<String>,
    pub metadata: Value,
}

impl SuperAgentStore {
    /// 记录一条进入系统的环境 Trigger 及其路由去向。
    ///
    /// 参数:
    /// - `input`: Trigger 载荷
    ///
    /// 返回:
    /// - 已落盘 Trigger 记录
    pub fn record_trigger(&self, input: NewTriggerEvent) -> Result<TriggerEvent> {
        let now = now_millis();
        let id = new_id("trg");
        let targets_json = serde_json::to_string(&input.target_member_ids)?;
        let metadata_json = serde_json::to_string(&input.metadata)?;
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO super_triggers
             (id, group_id, thread_id, source_channel, actor, content, disposition, route_path,
              reason, target_member_ids_json, metadata_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                id,
                input.group_id,
                input.thread_id,
                input.source_channel,
                input.actor,
                input.content,
                input.disposition.as_str(),
                input.route_path,
                input.reason,
                targets_json,
                metadata_json,
                now,
                now,
            ],
        )?;
        Ok(TriggerEvent {
            id,
            group_id: input.group_id,
            thread_id: input.thread_id,
            source_channel: input.source_channel,
            actor: input.actor,
            content: input.content,
            disposition: input.disposition,
            route_path: input.route_path,
            reason: input.reason,
            target_member_ids: input.target_member_ids,
            metadata: input.metadata,
            created_at: now,
            updated_at: now,
        })
    }

    /// 更新 Trigger 的去向与处理说明（例如从 queued 转为 batched / evaluated）。
    ///
    /// 参数:
    /// - `trigger_ids`: Trigger ID 列表
    /// - `disposition`: 新去向
    /// - `reason`: 可选新说明
    ///
    /// 返回:
    /// - 更新条数
    pub fn update_trigger_disposition(
        &self,
        trigger_ids: &[String],
        disposition: TriggerDisposition,
        reason: Option<&str>,
    ) -> Result<usize> {
        if trigger_ids.is_empty() {
            return Ok(0);
        }
        let now = now_millis();
        let conn = self.conn()?;
        let mut count = 0usize;
        for id in trigger_ids {
            count += if let Some(note) = reason {
                conn.execute(
                    "UPDATE super_triggers SET disposition = ?2, reason = ?3, updated_at = ?4 WHERE id = ?1",
                    params![id, disposition.as_str(), note, now],
                )?
            } else {
                conn.execute(
                    "UPDATE super_triggers SET disposition = ?2, updated_at = ?3 WHERE id = ?1",
                    params![id, disposition.as_str(), now],
                )?
            };
        }
        Ok(count)
    }

    /// 按 ID 读取单条 Trigger（供 Chain of Recall 使用）。
    ///
    /// 参数:
    /// - `trigger_id`: Trigger 标识
    ///
    /// 返回:
    /// - Trigger 记录
    pub fn get_trigger(&self, trigger_id: &str) -> Result<Option<TriggerEvent>> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT id, group_id, thread_id, source_channel, actor, content, disposition,
                    route_path, reason, target_member_ids_json, metadata_json, created_at, updated_at
             FROM super_triggers WHERE id = ?1",
            params![trigger_id],
            map_trigger,
        )
        .optional()
        .map_err(Into::into)
    }

    /// 列出群组最近的 Trigger 审计记录。
    ///
    /// 参数:
    /// - `group_id`: 群组标识
    /// - `limit`: 最大返回条数
    ///
    /// 返回:
    /// - 按创建时间倒序排列的 Trigger 列表
    pub fn list_triggers(&self, group_id: &str, limit: usize) -> Result<Vec<TriggerEvent>> {
        let conn = self.conn()?;
        let cap = limit.clamp(1, 200) as i64;
        let mut stmt = conn.prepare(
            "SELECT id, group_id, thread_id, source_channel, actor, content, disposition,
                    route_path, reason, target_member_ids_json, metadata_json, created_at, updated_at
             FROM super_triggers
             WHERE group_id = ?1
             ORDER BY created_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![group_id, cap], map_trigger)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// 取出群组内等待 Leader 批处理的 Trigger 队列（按到达顺序）。
    ///
    /// 参数:
    /// - `group_id`: 群组标识
    /// - `limit`: 单批上限
    ///
    /// 返回:
    /// - 待批处理的 Trigger 列表
    pub fn list_queued_leader_triggers(
        &self,
        group_id: &str,
        limit: usize,
    ) -> Result<Vec<TriggerEvent>> {
        let conn = self.conn()?;
        let cap = limit.clamp(1, 64) as i64;
        let mut stmt = conn.prepare(
            "SELECT id, group_id, thread_id, source_channel, actor, content, disposition,
                    route_path, reason, target_member_ids_json, metadata_json, created_at, updated_at
             FROM super_triggers
             WHERE group_id = ?1 AND disposition = 'queued' AND route_path = 'slow_path'
             ORDER BY created_at ASC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![group_id, cap], map_trigger)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// 绑定或更新外部渠道与群组的路由映射。
    ///
    /// 参数:
    /// - `group_id`: 目标群组标识
    /// - `channel`: 渠道类型（weixin/feishu/qq/onebot）
    /// - `external_chat_id`: 外部聊天/用户 ID（`*` 表示该渠道全部入站）
    /// - `label`: 可读标签
    /// - `admission_mode`: 准入策略（auto / explicit_only / ignore）
    /// - `enabled`: 是否启用
    ///
    /// 返回:
    /// - 生效的渠道绑定记录
    pub fn upsert_channel_binding(
        &self,
        group_id: &str,
        channel: &str,
        external_chat_id: &str,
        label: &str,
        admission_mode: &str,
        enabled: bool,
    ) -> Result<ChannelBinding> {
        let now = now_millis();
        let id = new_id("chn");
        let channel = channel.trim().to_ascii_lowercase();
        let chat_id = if external_chat_id.trim().is_empty() {
            "*".to_string()
        } else {
            external_chat_id.trim().to_string()
        };
        let display = if label.trim().is_empty() {
            format!("{channel}:{chat_id}")
        } else {
            label.trim().to_string()
        };
        let mode = match admission_mode.trim().to_ascii_lowercase().as_str() {
            "explicit_only" | "ignore" => admission_mode.trim().to_ascii_lowercase(),
            _ => "auto".to_string(),
        };
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO super_channel_bindings
             (id, group_id, channel, external_chat_id, label, admission_mode, enabled, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(channel, external_chat_id) DO UPDATE SET
               group_id = excluded.group_id,
               label = excluded.label,
               admission_mode = excluded.admission_mode,
               enabled = excluded.enabled",
            params![id, group_id, channel, chat_id, display, mode, i64::from(enabled), now],
        )?;
        conn.query_row(
            "SELECT id, group_id, channel, external_chat_id, label, admission_mode,
                    enabled, last_inbound_at, last_outbound_at, created_at
             FROM super_channel_bindings
             WHERE channel = ?1 AND external_chat_id = ?2",
            params![channel, chat_id],
            map_channel_binding,
        )
        .map_err(Into::into)
    }

    /// 移除一条渠道绑定。
    ///
    /// 参数:
    /// - `binding_id`: 绑定标识
    ///
    /// 返回:
    /// - 是否移除了记录
    pub fn delete_channel_binding(&self, binding_id: &str) -> Result<bool> {
        let conn = self.conn()?;
        let affected = conn.execute(
            "DELETE FROM super_channel_bindings WHERE id = ?1",
            params![binding_id],
        )?;
        Ok(affected > 0)
    }

    /// 列出指定群组或全局的渠道绑定。
    ///
    /// 参数:
    /// - `group_id`: 可选群组标识
    ///
    /// 返回:
    /// - 渠道绑定列表
    pub fn list_channel_bindings(&self, group_id: Option<&str>) -> Result<Vec<ChannelBinding>> {
        let conn = self.conn()?;
        let sql = if group_id.is_some() {
            "SELECT id, group_id, channel, external_chat_id, label, admission_mode,
                    enabled, last_inbound_at, last_outbound_at, created_at
             FROM super_channel_bindings
             WHERE group_id = ?1
             ORDER BY created_at ASC"
        } else {
            "SELECT id, group_id, channel, external_chat_id, label, admission_mode,
                    enabled, last_inbound_at, last_outbound_at, created_at
             FROM super_channel_bindings
             ORDER BY created_at ASC"
        };
        let mut stmt = conn.prepare(sql)?;
        let rows = match group_id {
            Some(gid) => stmt.query_map(params![gid], map_channel_binding)?,
            None => stmt.query_map([], map_channel_binding)?,
        };
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// 根据入站渠道与外部会话 ID 查找最精确的启用绑定（精确匹配优先于 `*` 通配）。
    ///
    /// 参数:
    /// - `channel`: 渠道标识
    /// - `external_chat_id`: 外部会话或用户标识
    ///
    /// 返回:
    /// - 命中的渠道绑定
    pub fn resolve_channel_binding(
        &self,
        channel: &str,
        external_chat_id: &str,
    ) -> Result<Option<ChannelBinding>> {
        let conn = self.conn()?;
        let channel = channel.trim().to_ascii_lowercase();
        conn.query_row(
            "SELECT id, group_id, channel, external_chat_id, label, admission_mode,
                    enabled, last_inbound_at, last_outbound_at, created_at
             FROM super_channel_bindings
             WHERE channel = ?1 AND enabled = 1 AND (external_chat_id = ?2 OR external_chat_id = '*')
             ORDER BY CASE WHEN external_chat_id = ?2 THEN 0 ELSE 1 END
             LIMIT 1",
            params![channel, external_chat_id],
            map_channel_binding,
        )
        .optional()
        .map_err(Into::into)
    }

    /// 更新渠道绑定的最近入站或出站时间戳。
    ///
    /// 参数:
    /// - `binding_id`: 绑定标识
    /// - `is_inbound`: true 更新入站时间，false 更新出站时间
    ///
    /// 返回:
    /// - 更新是否成功
    pub fn touch_channel_binding(&self, binding_id: &str, is_inbound: bool) -> Result<()> {
        let conn = self.conn()?;
        let column = if is_inbound {
            "last_inbound_at"
        } else {
            "last_outbound_at"
        };
        let sql = format!("UPDATE super_channel_bindings SET {column} = ?2 WHERE id = ?1");
        conn.execute(&sql, params![binding_id, now_millis()])?;
        Ok(())
    }

    /// 记录一条 Leader 向外部渠道递交成果的回执。
    ///
    /// 参数:
    /// - `group_id`: 群组标识
    /// - `task_id`: 关联任务标识
    /// - `channel`: 目标渠道
    /// - `target`: 目标地址描述
    /// - `status`: 递交状态（sent / simulated / failed / skipped）
    /// - `preview`: 递交正文摘要
    /// - `error`: 可选失败原因
    ///
    /// 返回:
    /// - 回执记录
    pub fn record_delivery_receipt(
        &self,
        group_id: &str,
        task_id: Option<&str>,
        channel: &str,
        target: &str,
        status: &str,
        preview: &str,
        error: Option<&str>,
    ) -> Result<ChannelDeliveryReceipt> {
        let now = now_millis();
        let id = new_id("dlv");
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO super_deliveries
             (id, group_id, task_id, channel, target, status, preview, error, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![id, group_id, task_id, channel, target, status, preview, error, now],
        )?;
        Ok(ChannelDeliveryReceipt {
            id,
            group_id: group_id.to_string(),
            task_id: task_id.map(ToOwned::to_owned),
            channel: channel.to_string(),
            target: target.to_string(),
            status: status.to_string(),
            preview: preview.to_string(),
            error: error.map(ToOwned::to_owned),
            created_at: now,
        })
    }

    /// 列出群组最近的渠道递交回执。
    ///
    /// 参数:
    /// - `group_id`: 群组标识
    /// - `limit`: 最大返回条数
    ///
    /// 返回:
    /// - 回执列表
    pub fn list_delivery_receipts(
        &self,
        group_id: &str,
        limit: usize,
    ) -> Result<Vec<ChannelDeliveryReceipt>> {
        let conn = self.conn()?;
        let cap = limit.clamp(1, 200) as i64;
        let mut stmt = conn.prepare(
            "SELECT id, group_id, task_id, channel, target, status, preview, error, created_at
             FROM super_deliveries
             WHERE group_id = ?1
             ORDER BY created_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![group_id, cap], map_delivery)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }
}

/// 映射 Trigger 行。
fn map_trigger(row: &rusqlite::Row<'_>) -> rusqlite::Result<TriggerEvent> {
    let disposition_raw: String = row.get(6)?;
    let targets_raw: String = row.get(9)?;
    let metadata_raw: String = row.get(10)?;
    Ok(TriggerEvent {
        id: row.get(0)?,
        group_id: row.get(1)?,
        thread_id: row.get(2)?,
        source_channel: row.get(3)?,
        actor: row.get(4)?,
        content: row.get(5)?,
        disposition: TriggerDisposition::parse(&disposition_raw),
        route_path: row.get(7)?,
        reason: row.get(8)?,
        target_member_ids: serde_json::from_str(&targets_raw).unwrap_or_default(),
        metadata: serde_json::from_str(&metadata_raw)
            .unwrap_or_else(|_| Value::Object(Default::default())),
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

/// 映射渠道绑定行。
fn map_channel_binding(row: &rusqlite::Row<'_>) -> rusqlite::Result<ChannelBinding> {
    let enabled: i64 = row.get(6)?;
    Ok(ChannelBinding {
        id: row.get(0)?,
        group_id: row.get(1)?,
        channel: row.get(2)?,
        external_chat_id: row.get(3)?,
        label: row.get(4)?,
        admission_mode: row.get(5)?,
        enabled: enabled != 0,
        last_inbound_at: row.get(7)?,
        last_outbound_at: row.get(8)?,
        created_at: row.get(9)?,
    })
}

/// 映射渠道递交回执行。
fn map_delivery(row: &rusqlite::Row<'_>) -> rusqlite::Result<ChannelDeliveryReceipt> {
    Ok(ChannelDeliveryReceipt {
        id: row.get(0)?,
        group_id: row.get(1)?,
        task_id: row.get(2)?,
        channel: row.get(3)?,
        target: row.get(4)?,
        status: row.get(5)?,
        preview: row.get(6)?,
        error: row.get(7)?,
        created_at: row.get(8)?,
    })
}
