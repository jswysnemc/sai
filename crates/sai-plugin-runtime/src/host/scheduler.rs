use crate::{manifest::validate_identifier, Capabilities};
use anyhow::{bail, ensure, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MAX_SCHEDULE_ARGUMENTS: usize = 16 * 1024;
pub const MAX_SCHEDULED_TASKS: usize = 128;
pub const MAX_SCHEDULE_LIST_OFFSET: usize = 256;
pub const MAX_ACTIVE_TASKS: usize = 32;

/// 【插件调度】【分页查询】每页限制数量，任务文本不会让列表无限增长。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ScheduleListOptions {
    pub offset: usize,
    pub limit: usize,
}

impl Default for ScheduleListOptions {
    /// 【插件调度】【默认分页】从第一条开始，每次最多返回十六条。
    /// @returns 默认查询选项
    fn default() -> Self {
        Self {
            offset: 0,
            limit: 16,
        }
    }
}

/// 【插件调度】【请求】在指定 Unix 秒执行本插件命令，不接受程序路径或其他插件标识。
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ScheduleRequest {
    pub due_at: i64,
    pub command: String,
    #[serde(default)]
    pub arguments: String,
}

/// 【插件调度】【状态】取消请求与工作线程确认分开，终态不会自动重试。
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScheduledStatus {
    Scheduled,
    Running,
    Cancelling,
    Cancelled,
    Completed,
    Failed,
}

/// 【插件调度】【公开记录】只返回本插件任务资料，不公开宿主路径、源码或授权快照。
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ScheduledTask {
    pub id: String,
    pub command: String,
    pub arguments: String,
    pub due_at: i64,
    pub status: ScheduledStatus,
    pub pid: Option<u32>,
    pub created_at: i64,
    pub finished_at: Option<i64>,
    pub output: Option<String>,
    pub output_truncated: bool,
    pub error: Option<String>,
}

/// 【插件调度】【操作】调度和恢复均属于写入，查询不启动工作进程。
#[derive(Clone, Debug)]
pub enum SchedulerRequest {
    Schedule(ScheduleRequest),
    List(ScheduleListOptions),
    Get(String),
    Cancel(String),
    Resume(String),
}

#[derive(Debug)]
pub enum SchedulerResponse {
    Task(Option<ScheduledTask>),
    Tasks(Vec<ScheduledTask>),
    Changed(bool),
}

impl ScheduleRequest {
    /// 【插件调度】【参数校验】限制命令、文本和可表示日期，过去时间表示尽快执行。
    /// @returns 请求格式合法时成功
    pub fn validate(&self) -> Result<()> {
        validate_identifier(&self.command, 48)?;
        ensure!(
            (0..=253_402_300_799).contains(&self.due_at),
            "scheduled time is outside years 1970-9999"
        );
        ensure!(
            self.arguments.len() <= MAX_SCHEDULE_ARGUMENTS,
            "scheduled arguments exceed 16 KiB"
        );
        Ok(())
    }
}

impl ScheduledStatus {
    /// 【插件调度】【终态判断】已完成、失败或确认取消的任务不再执行。
    /// @returns 是否仍占用活动任务配额
    pub fn is_active(self) -> bool {
        matches!(self, Self::Scheduled | Self::Running | Self::Cancelling)
    }
}

impl ScheduledTask {
    /// 【插件调度】【结果校验】约束持久记录和宿主结果中的字符串与时间范围。
    /// @returns 记录可以传递给 Lua 时成功
    pub fn validate(&self) -> Result<()> {
        validate_scheduled_id(&self.id)?;
        ScheduleRequest {
            due_at: self.due_at,
            command: self.command.clone(),
            arguments: self.arguments.clone(),
        }
        .validate()?;
        ensure!(
            self.output
                .as_ref()
                .is_none_or(|value| value.len() <= MAX_SCHEDULE_ARGUMENTS),
            "scheduled output exceeds 16 KiB"
        );
        ensure!(
            self.error.as_ref().is_none_or(|value| value.len() <= 4096),
            "scheduled error exceeds 4096 bytes"
        );
        ensure!(self.pid != Some(0), "invalid scheduled process id");
        ensure!(
            (0..=253_402_300_799).contains(&self.created_at)
                && self
                    .finished_at
                    .is_none_or(|value| (0..=253_402_300_799).contains(&value)),
            "invalid scheduled record timestamp"
        );
        ensure!(
            self.status.is_active() == self.finished_at.is_none(),
            "scheduled status and completion time disagree"
        );
        Ok(())
    }
}

impl SchedulerRequest {
    /// 【插件调度】【授权检查】读取与写入都要求独立调度授权，变更另需可信写入权限。
    /// @param capabilities 有效能力；allow_writes 为 Rust 调用权限
    /// @returns 可以进入宿主时成功
    pub fn authorize(&self, capabilities: &Capabilities, allow_writes: bool) -> Result<()> {
        ensure!(
            capabilities.system.schedule,
            "plugin scheduling is not allowed"
        );
        if !matches!(self, Self::List(_) | Self::Get(_)) {
            ensure!(
                allow_writes,
                "read-only plugin callback cannot change scheduled tasks"
            );
        }
        match self {
            Self::Schedule(request) => request.validate(),
            Self::Get(id) | Self::Cancel(id) | Self::Resume(id) => validate_scheduled_id(id),
            Self::List(options) => {
                ensure!(
                    options.offset <= MAX_SCHEDULE_LIST_OFFSET && (1..=16).contains(&options.limit),
                    "scheduled list requires offset 0-256 and limit 1-16"
                );
                Ok(())
            }
        }
    }

    /// 【插件调度】【响应匹配】拒绝错误形状、重复任务和与请求不符的宿主响应。
    /// @param response 宿主返回值
    /// @returns 响应满足本次操作契约时成功
    pub fn validate_response(&self, response: &SchedulerResponse) -> Result<()> {
        match (self, response) {
            (Self::Schedule(request), SchedulerResponse::Task(Some(task))) => {
                task.validate()?;
                ensure!(
                    task.command == request.command
                        && task.arguments == request.arguments
                        && task.due_at == request.due_at,
                    "scheduler host returned a different task"
                );
            }
            (Self::Get(id) | Self::Resume(id), SchedulerResponse::Task(task)) => {
                if let Some(task) = task {
                    task.validate()?;
                    ensure!(
                        task.id == *id,
                        "scheduler host returned a different task id"
                    );
                }
            }
            (Self::List(options), SchedulerResponse::Tasks(tasks)) => {
                ensure!(
                    tasks.len() <= options.limit,
                    "scheduler host returned too many tasks"
                );
                let mut ids = BTreeSet::new();
                for task in tasks {
                    task.validate()?;
                    ensure!(
                        ids.insert(&task.id),
                        "scheduler host returned duplicate tasks"
                    );
                }
            }
            (Self::Cancel(_), SchedulerResponse::Changed(_)) => {}
            _ => bail!("scheduler host returned an inconsistent response"),
        }
        Ok(())
    }
}

/// 【插件调度】【标识校验】任务使用宿主生成的固定长度十六进制标识。
/// @param id 任务标识
/// @returns 标识合法时成功
pub fn validate_scheduled_id(id: &str) -> Result<()> {
    let value = id.strip_prefix("job-").unwrap_or_default();
    ensure!(
        value.len() == 32
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "invalid scheduled task id"
    );
    Ok(())
}
