use crate::paths::SaiPaths;
use anyhow::{ensure, Result};
use sai_plugin_runtime::host::{
    ScheduleRequest, ScheduledStatus, ScheduledTask, MAX_SCHEDULE_ARGUMENTS,
};
use serde::{Deserialize, Serialize};

/// 【插件调度】【持久记录】保存可信归属与执行快照摘要，不复制插件设置或凭据。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct JobRecord {
    pub version: u32,
    pub plugin: String,
    pub task: ScheduledTask,
    pub revision: String,
    pub paths: SaiPaths,
    pub workdir: String,
    pub launch: String,
}

impl JobRecord {
    /// 【插件调度】【记录创建】生成独立随机标识，任务在发布前不具有工作进程。
    /// @param plugin 插件；revision 为源码与授权摘要；request 为请求；paths 为宿主路径；workdir 为可信目录
    /// @returns 尚未启动的持久任务记录
    pub fn new(
        plugin: &str,
        revision: &str,
        request: ScheduleRequest,
        paths: &SaiPaths,
        workdir: String,
    ) -> Self {
        Self {
            version: 1,
            plugin: plugin.into(),
            revision: revision.into(),
            paths: paths.clone(),
            workdir,
            launch: String::new(),
            task: ScheduledTask {
                id: format!("job-{}", uuid::Uuid::new_v4().simple()),
                command: request.command,
                arguments: request.arguments,
                due_at: request.due_at,
                status: ScheduledStatus::Scheduled,
                pid: None,
                created_at: chrono::Utc::now().timestamp(),
                finished_at: None,
                output: None,
                output_truncated: false,
                error: None,
            },
        }
    }

    /// 【插件调度】【归属校验】拒绝跨插件复制、损坏版本及无效执行身份。
    /// @param plugin 目录绑定插件；id 为请求的任务标识
    /// @returns 记录属于当前命名空间时成功
    pub fn validate(&self, plugin: &str, id: &str) -> Result<()> {
        ensure!(
            self.version == 1 && self.plugin == plugin && self.task.id == id,
            "scheduled record ownership or version is invalid"
        );
        self.task.validate()?;
        ensure!(
            self.revision.len() == 64 && self.revision.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "invalid scheduled source revision"
        );
        ensure!(
            self.launch.is_empty()
                || (self.launch.len() == 32
                    && self.launch.bytes().all(|byte| byte.is_ascii_hexdigit())),
            "invalid scheduled launch identity"
        );
        ensure!(
            std::path::Path::new(&self.workdir).is_absolute(),
            "scheduled working directory must be absolute"
        );
        Ok(())
    }

    /// 【插件调度】【终态记录】保留有界输出或错误，取消优先于同时到达的成功结果。
    /// @param result 命令结果；None 表示取消执行
    /// @returns 无，原地更新记录
    pub fn finish(&mut self, result: Option<Result<String>>) {
        self.task.finished_at = Some(chrono::Utc::now().timestamp());
        if matches!(
            self.task.status,
            ScheduledStatus::Cancelling | ScheduledStatus::Cancelled
        ) || result.is_none()
        {
            self.task.status = ScheduledStatus::Cancelled;
            return;
        }
        match result.expect("checked result") {
            Ok(output) => {
                self.task.status = ScheduledStatus::Completed;
                self.task.output_truncated = output.len() > MAX_SCHEDULE_ARGUMENTS;
                self.task.output = Some(clip(&output, MAX_SCHEDULE_ARGUMENTS));
            }
            Err(error) => {
                self.task.status = ScheduledStatus::Failed;
                self.task.error = Some(clip(&format!("{error:#}"), 4096));
            }
        }
    }
}

/// 【插件调度】【文本截断】按 UTF-8 边界限制持久结果，避免截断多字节字符。
/// @param value 文本；limit 为最大字节数
/// @returns 有界文本
fn clip(value: &str, limit: usize) -> String {
    let mut end = value.len().min(limit);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].into()
}
