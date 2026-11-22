use super::SuperAgentStore;
use crate::super_agent::model::{
    new_id, now_millis, Group, GroupMember, GroupStatus, GroupThread, MemberRole, MemberStatus,
};
use anyhow::{anyhow, Result};
use rusqlite::{params, OptionalExtension};

impl SuperAgentStore {
    /// 创建群组并同步初始化常驻 Leader 成员与默认主话题 Thread。
    ///
    /// 参数:
    /// - `name`: 群组名称
    /// - `description`: 群组简介
    /// - `workspace_path`: 绑定的工作区目录
    /// - `leader_name`: Leader 显示名称
    /// - `leader_model`: Leader 使用模型标签
    ///
    /// 返回:
    /// - 新建群组与 Leader 成员
    pub fn create_group(
        &self,
        name: &str,
        description: &str,
        workspace_path: &str,
        leader_name: &str,
        leader_model: &str,
    ) -> Result<(Group, GroupMember)> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let now = now_millis();
        let group_id = new_id("grp");
        let leader_id = new_id("ldr");
        let thread_id = new_id("thr");
        let clean_name = name.trim();
        let group_name = if clean_name.is_empty() {
            "Super-Agent Workspace"
        } else {
            clean_name
        };
        // 1. 写入群组主记录
        tx.execute(
            "INSERT INTO super_groups
             (id, name, description, workspace_path, status, leader_member_id, default_thread_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                group_id,
                group_name,
                description.trim(),
                workspace_path.trim(),
                GroupStatus::Active.as_str(),
                leader_id,
                thread_id,
                now,
                now,
            ],
        )?;
        // 2. 创建群组默认主话题线程
        tx.execute(
            "INSERT INTO super_threads
             (id, group_id, title, channel, external_ref, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'group', NULL, 'open', ?4, ?5)",
            params![thread_id, group_id, "Main Lobby", now, now],
        )?;
        // 3. 创建常驻 Leader 成员
        let leader_label = if leader_name.trim().is_empty() {
            "Leader"
        } else {
            leader_name.trim()
        };
        tx.execute(
            "INSERT INTO super_members
             (id, group_id, name, role, status, specialty, subagent_type, model_label, workdir,
              worktree_branch, active_task_id, last_activity, last_heartbeat_at, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'leader', 'idle', ?4, 'leader', ?5, ?6, NULL, NULL, '', ?7, ?8, ?9)",
            params![
                leader_id,
                group_id,
                leader_label,
                "Router & Orchestrator",
                leader_model.trim(),
                workspace_path.trim(),
                now,
                now,
                now,
            ],
        )?;
        tx.commit()?;
        let group = Group {
            id: group_id.clone(),
            name: group_name.to_string(),
            description: description.trim().to_string(),
            workspace_path: workspace_path.trim().to_string(),
            status: GroupStatus::Active,
            leader_member_id: leader_id.clone(),
            default_thread_id: thread_id,
            created_at: now,
            updated_at: now,
        };
        let leader = GroupMember {
            id: leader_id,
            group_id,
            name: leader_label.to_string(),
            role: MemberRole::Leader,
            status: MemberStatus::Idle,
            specialty: "Router & Orchestrator".to_string(),
            subagent_type: "leader".to_string(),
            model_label: leader_model.trim().to_string(),
            workdir: workspace_path.trim().to_string(),
            worktree_branch: None,
            active_task_id: None,
            last_activity: String::new(),
            last_heartbeat_at: now,
            created_at: now,
            updated_at: now,
        };
        Ok((group, leader))
    }

    /// 列出全部群组（按最近更新时间倒序）。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 群组列表
    pub fn list_groups(&self) -> Result<Vec<Group>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, description, workspace_path, status, leader_member_id,
                    default_thread_id, created_at, updated_at
             FROM super_groups
             ORDER BY updated_at DESC, created_at DESC",
        )?;
        let rows = stmt.query_map([], map_group)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// 按标识读取单个群组。
    ///
    /// 参数:
    /// - `group_id`: 群组标识
    ///
    /// 返回:
    /// - 匹配的群组，不存在时返回 None
    pub fn get_group(&self, group_id: &str) -> Result<Option<Group>> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT id, name, description, workspace_path, status, leader_member_id,
                    default_thread_id, created_at, updated_at
             FROM super_groups WHERE id = ?1",
            params![group_id],
            map_group,
        )
        .optional()
        .map_err(Into::into)
    }

    /// 按名称或前缀查找群组。
    ///
    /// 参数:
    /// - `query`: 群组 ID 或名称
    ///
    /// 返回:
    /// - 首个匹配的群组
    #[allow(dead_code)]
    pub fn find_group(&self, query: &str) -> Result<Option<Group>> {
        let needle = query.trim();
        if needle.is_empty() {
            return Ok(None);
        }
        if let Some(group) = self.get_group(needle)? {
            return Ok(Some(group));
        }
        let lowered = needle.to_ascii_lowercase();
        let groups = self.list_groups()?;
        Ok(groups.into_iter().find(|group| {
            group.name.to_ascii_lowercase() == lowered
                || group.id.starts_with(needle)
                || group.name.to_ascii_lowercase().contains(&lowered)
        }))
    }

    /// 更新群组名称、简介或状态。
    ///
    /// 参数:
    /// - `group_id`: 群组标识
    /// - `name`: 可选新名称
    /// - `description`: 可选新简介
    /// - `status`: 可选新状态
    ///
    /// 返回:
    /// - 更新后的群组
    pub fn update_group(
        &self,
        group_id: &str,
        name: Option<&str>,
        description: Option<&str>,
        status: Option<GroupStatus>,
    ) -> Result<Group> {
        let mut group = self
            .get_group(group_id)?
            .ok_or_else(|| anyhow!("group not found: {group_id}"))?;
        if let Some(value) = name.map(str::trim).filter(|value| !value.is_empty()) {
            group.name = value.to_string();
        }
        if let Some(value) = description {
            group.description = value.trim().to_string();
        }
        if let Some(value) = status {
            group.status = value;
        }
        group.updated_at = now_millis();
        let conn = self.conn()?;
        conn.execute(
            "UPDATE super_groups
             SET name = ?2, description = ?3, status = ?4, updated_at = ?5
             WHERE id = ?1",
            params![
                group.id,
                group.name,
                group.description,
                group.status.as_str(),
                group.updated_at,
            ],
        )?;
        Ok(group)
    }

    /// 删除指定群组及其关联记录。
    ///
    /// 参数:
    /// - `group_id`: 群组标识
    ///
    /// 返回:
    /// - 是否实际删除了记录
    pub fn delete_group(&self, group_id: &str) -> Result<bool> {
        let conn = self.conn()?;
        let affected = conn.execute("DELETE FROM super_groups WHERE id = ?1", params![group_id])?;
        Ok(affected > 0)
    }

    /// 刷新群组最近活跃时间戳。
    ///
    /// 参数:
    /// - `group_id`: 群组标识
    ///
    /// 返回:
    /// - 更新是否成功
    pub fn touch_group(&self, group_id: &str) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE super_groups SET updated_at = ?2 WHERE id = ?1",
            params![group_id, now_millis()],
        )?;
        Ok(())
    }

    /// 在群组中创建新的 Worker(Runner) 成员。
    ///
    /// 参数:
    /// - `group_id`: 所属群组标识
    /// - `name`: Runner 名称
    /// - `specialty`: 专长或职责说明
    /// - `subagent_type`: 子代理档案标识
    /// - `model_label`: 模型展示标签
    /// - `workdir`: 执行目录
    /// - `worktree_branch`: 可选隔离分支名
    ///
    /// 返回:
    /// - 新建 Worker 成员记录
    pub fn create_worker_member(
        &self,
        group_id: &str,
        name: &str,
        specialty: &str,
        subagent_type: &str,
        model_label: &str,
        workdir: &str,
        worktree_branch: Option<&str>,
    ) -> Result<GroupMember> {
        let now = now_millis();
        let id = new_id("run");
        let clean_name = name.trim();
        let member_name = if clean_name.is_empty() {
            format!("runner-{}", &id[4..8])
        } else {
            clean_name.to_string()
        };
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO super_members
             (id, group_id, name, role, status, specialty, subagent_type, model_label, workdir,
              worktree_branch, active_task_id, last_activity, last_heartbeat_at, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'worker', 'idle', ?4, ?5, ?6, ?7, ?8, NULL, '', ?9, ?10, ?11)",
            params![
                id,
                group_id,
                member_name,
                specialty.trim(),
                if subagent_type.trim().is_empty() {
                    "general"
                } else {
                    subagent_type.trim()
                },
                model_label.trim(),
                workdir.trim(),
                worktree_branch,
                now,
                now,
                now,
            ],
        )?;
        self.touch_group(group_id)?;
        Ok(GroupMember {
            id,
            group_id: group_id.to_string(),
            name: member_name,
            role: MemberRole::Worker,
            status: MemberStatus::Idle,
            specialty: specialty.trim().to_string(),
            subagent_type: if subagent_type.trim().is_empty() {
                "general".to_string()
            } else {
                subagent_type.trim().to_string()
            },
            model_label: model_label.trim().to_string(),
            workdir: workdir.trim().to_string(),
            worktree_branch: worktree_branch.map(ToOwned::to_owned),
            active_task_id: None,
            last_activity: String::new(),
            last_heartbeat_at: now,
            created_at: now,
            updated_at: now,
        })
    }

    /// 列出指定群组的全部成员（Leader 排在首位）。
    ///
    /// 参数:
    /// - `group_id`: 群组标识
    ///
    /// 返回:
    /// - 成员列表
    pub fn list_members(&self, group_id: &str) -> Result<Vec<GroupMember>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, group_id, name, role, status, specialty, subagent_type, model_label,
                    workdir, worktree_branch, active_task_id, last_activity, last_heartbeat_at,
                    created_at, updated_at
             FROM super_members
             WHERE group_id = ?1
             ORDER BY CASE role WHEN 'leader' THEN 0 ELSE 1 END, created_at ASC",
        )?;
        let rows = stmt.query_map(params![group_id], map_member)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// 按成员标识读取成员记录。
    ///
    /// 参数:
    /// - `member_id`: 成员标识
    ///
    /// 返回:
    /// - 成员快照
    pub fn get_member(&self, member_id: &str) -> Result<Option<GroupMember>> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT id, group_id, name, role, status, specialty, subagent_type, model_label,
                    workdir, worktree_branch, active_task_id, last_activity, last_heartbeat_at,
                    created_at, updated_at
             FROM super_members WHERE id = ?1",
            params![member_id],
            map_member,
        )
        .optional()
        .map_err(Into::into)
    }

    /// 按名称或 ID 在群组内定位成员（供 `@mention` 路由使用）。
    ///
    /// 参数:
    /// - `group_id`: 群组标识
    /// - `name_or_id`: 成员名或 ID
    ///
    /// 返回:
    /// - 匹配的成员
    pub fn find_member(&self, group_id: &str, name_or_id: &str) -> Result<Option<GroupMember>> {
        let needle = name_or_id.trim().trim_start_matches('@');
        if needle.is_empty() {
            return Ok(None);
        }
        let lowered = needle.to_ascii_lowercase();
        let members = self.list_members(group_id)?;
        Ok(members.into_iter().find(|member| {
            member.id == needle
                || member.name.to_ascii_lowercase() == lowered
                || (lowered == "leader" && member.role == MemberRole::Leader)
        }))
    }

    /// 更新成员状态、当前活动说明、当前任务与心跳时间。
    ///
    /// 参数:
    /// - `member_id`: 成员标识
    /// - `status`: 新状态
    /// - `last_activity`: 可选活动描述
    /// - `active_task_id`: 可选当前任务覆盖（Some(None) 表示清空）
    ///
    /// 返回:
    /// - 更新后的成员快照
    pub fn update_member_state(
        &self,
        member_id: &str,
        status: MemberStatus,
        last_activity: Option<&str>,
        active_task_id: Option<Option<&str>>,
    ) -> Result<GroupMember> {
        let mut member = self
            .get_member(member_id)?
            .ok_or_else(|| anyhow!("member not found: {member_id}"))?;
        let now = now_millis();
        member.status = status;
        if let Some(activity) = last_activity {
            member.last_activity = activity.trim().to_string();
        }
        if let Some(task) = active_task_id {
            member.active_task_id = task.map(ToOwned::to_owned);
        }
        member.last_heartbeat_at = now;
        member.updated_at = now;
        let conn = self.conn()?;
        conn.execute(
            "UPDATE super_members
             SET status = ?2, last_activity = ?3, active_task_id = ?4,
                 last_heartbeat_at = ?5, updated_at = ?6
             WHERE id = ?1",
            params![
                member.id,
                member.status.as_str(),
                member.last_activity,
                member.active_task_id,
                member.last_heartbeat_at,
                member.updated_at,
            ],
        )?;
        Ok(member)
    }

    /// 更新成员工作目录与 worktree 分支信息。
    ///
    /// 参数:
    /// - `member_id`: 成员标识
    /// - `workdir`: 实际运行目录
    /// - `worktree_branch`: 隔离分支名
    ///
    /// 返回:
    /// - 更新是否成功
    pub fn update_member_workdir(
        &self,
        member_id: &str,
        workdir: &str,
        worktree_branch: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE super_members
             SET workdir = ?2, worktree_branch = ?3, updated_at = ?4
             WHERE id = ?1",
            params![member_id, workdir, worktree_branch, now_millis()],
        )?;
        Ok(())
    }

    /// 获取群组内处于运行态的所有 Worker 成员（供 Watchdog 与并发控制读取）。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 全部运行中的 Worker 列表
    pub fn list_running_workers(&self) -> Result<Vec<GroupMember>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, group_id, name, role, status, specialty, subagent_type, model_label,
                    workdir, worktree_branch, active_task_id, last_activity, last_heartbeat_at,
                    created_at, updated_at
             FROM super_members
             WHERE role = 'worker' AND status = 'running'
             ORDER BY updated_at ASC",
        )?;
        let rows = stmt.query_map([], map_member)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// 获取或创建群组默认主线程。
    ///
    /// 参数:
    /// - `group_id`: 群组标识
    ///
    /// 返回:
    /// - 默认主线程记录
    #[allow(dead_code)]
    pub fn default_thread(&self, group_id: &str) -> Result<GroupThread> {
        let group = self
            .get_group(group_id)?
            .ok_or_else(|| anyhow!("group not found: {group_id}"))?;
        if let Some(thread) = self.get_thread(&group.default_thread_id)? {
            return Ok(thread);
        }
        self.create_thread(group_id, "Main Lobby", "group", None)
    }
}

/// 将 SQLite 行映射为 Group。
///
/// 参数:
/// - `row`: 查询结果行
///
/// 返回:
/// - Group 实体
fn map_group(row: &rusqlite::Row<'_>) -> rusqlite::Result<Group> {
    let status: String = row.get(4)?;
    Ok(Group {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        workspace_path: row.get(3)?,
        status: GroupStatus::parse(&status),
        leader_member_id: row.get(5)?,
        default_thread_id: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

/// 将 SQLite 行映射为 GroupMember。
///
/// 参数:
/// - `row`: 查询结果行
///
/// 返回:
/// - GroupMember 实体
fn map_member(row: &rusqlite::Row<'_>) -> rusqlite::Result<GroupMember> {
    let role: String = row.get(3)?;
    let status: String = row.get(4)?;
    Ok(GroupMember {
        id: row.get(0)?,
        group_id: row.get(1)?,
        name: row.get(2)?,
        role: MemberRole::parse(&role),
        status: MemberStatus::parse(&status),
        specialty: row.get(5)?,
        subagent_type: row.get(6)?,
        model_label: row.get(7)?,
        workdir: row.get(8)?,
        worktree_branch: row.get(9)?,
        active_task_id: row.get(10)?,
        last_activity: row.get(11)?,
        last_heartbeat_at: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}
