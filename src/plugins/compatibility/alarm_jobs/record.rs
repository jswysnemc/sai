use anyhow::{ensure, Context, Result};
use sai_plugin_runtime::host::{ScheduledStatus, ScheduledTask};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum LegacyStatus {
    Scheduled,
    Ringing,
}

/// 【旧闹钟兼容】【记录格式】只接受旧工具已经写入的任务，不用于创建新闹钟。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LegacyRecord {
    pub id: String,
    pub label: String,
    pub time: String,
    pub audio_file: Option<PathBuf>,
    pub due_at: i64,
    pub pid: Option<u32>,
    pub status: LegacyStatus,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Flag {
    pub revision: String,
    pub status: ScheduledStatus,
    pub finished_at: Option<i64>,
    pub error: Option<String>,
}

impl LegacyRecord {
    /// 【旧闹钟兼容】【输入校验】限制旧记录大小、路径和原版生成的标识结构。
    /// @returns 记录可转换为通用任务时成功
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.id.len() <= 128 && self.time.len() <= 256 && self.label.len() <= 4096,
            "legacy alarm record exceeds text limits"
        );
        self.created_at()?;
        ensure!(
            (0..=253402300799).contains(&self.due_at) && self.pid != Some(0),
            "invalid legacy alarm time or process id"
        );
        if let Some(path) = &self.audio_file {
            ensure!(
                path.is_absolute() && path.as_os_str().len() <= 8192 && path.to_str().is_some(),
                "invalid legacy alarm audio path"
            );
        }
        self.task(self.status(), None, None)?.validate()
    }

    /// 【旧闹钟兼容】【创建时间】从原版标识取得创建秒数，避免重新计算提醒时间。
    /// @returns 创建时间
    fn created_at(&self) -> Result<i64> {
        let value = self
            .id
            .strip_prefix("alarm-")
            .context("invalid legacy alarm id")?;
        let (millis, parent) = value.split_once('-').context("invalid legacy alarm id")?;
        ensure!(
            !millis.is_empty()
                && millis.bytes().all(|byte| byte.is_ascii_digit())
                && !parent.is_empty()
                && parent.bytes().all(|byte| byte.is_ascii_digit()),
            "invalid legacy alarm id"
        );
        let millis: i64 = millis.parse()?;
        let parent: u32 = parent.parse()?;
        ensure!(
            (0..=253402300799999).contains(&millis) && parent > 0,
            "invalid legacy alarm id"
        );
        Ok(millis / 1000)
    }

    /// 【旧闹钟兼容】【内部标识】以带类别的摘要映射旧标识，不与用户可选路径混用。
    /// @returns 通用调度接口接受的任务标识
    pub fn task_id(&self) -> String {
        let digest = blake3::hash(format!("sai/legacy-alarm/{}", self.id).as_bytes())
            .to_hex()
            .to_string();
        format!("job-{}", &digest[..32])
    }

    /// 【旧闹钟兼容】【记录身份】状态更新不改变身份，其他字段变化不能沿用旧取消记录。
    /// @returns 记录内容摘要
    pub fn revision(&self) -> Result<String> {
        Ok(blake3::hash(&serde_json::to_vec(&(
            &self.id,
            &self.time,
            &self.label,
            &self.audio_file,
            self.due_at,
            self.pid,
        ))?)
        .to_hex()
        .to_string())
    }

    /// 【旧闹钟兼容】【状态转换】将旧的响铃状态映射为通用运行状态。
    /// @returns 通用状态
    pub fn status(&self) -> ScheduledStatus {
        match self.status {
            LegacyStatus::Scheduled => ScheduledStatus::Scheduled,
            LegacyStatus::Ringing => ScheduledStatus::Running,
        }
    }

    /// 【旧闹钟兼容】【业务参数】只把旧任务本身的参数交给 Lua，不传递应用配置。
    /// @returns 提醒命令参数
    pub fn arguments(&self) -> Result<String> {
        let audio = self
            .audio_file
            .as_ref()
            .map(|path| dunce::simplified(path).to_string_lossy().into_owned());
        Ok(serde_json::to_string(
            &serde_json::json!({"time":self.time,"label":self.label,"audio_file":audio,"legacy_id":self.id}),
        )?)
    }

    /// 【旧闹钟兼容】【公开投影】保留原到期时间和 PID，旧公开标识存于提醒参数。
    /// @param status 当前观察状态；finished_at 为终态观察时间；error 为有界错误
    /// @returns 通用任务记录
    pub fn task(
        &self,
        status: ScheduledStatus,
        finished_at: Option<i64>,
        error: Option<String>,
    ) -> Result<ScheduledTask> {
        Ok(ScheduledTask {
            id: self.task_id(),
            command: "deliver".into(),
            arguments: self.arguments()?,
            due_at: self.due_at,
            status,
            pid: self.pid,
            created_at: self.created_at()?,
            finished_at,
            output: None,
            output_truncated: false,
            error,
        })
    }
}

impl Flag {
    /// 【旧闹钟兼容】【独立状态】状态只写入兼容目录，不改写旧进程共享的 alarms.json。
    /// @param record 旧记录；status 为状态；error 为错误
    /// @returns 有界状态记录
    pub fn new(
        record: &LegacyRecord,
        status: ScheduledStatus,
        error: Option<&str>,
    ) -> Result<Self> {
        let error = error.map(|text| {
            let mut end = text.len().min(4096);
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            text[..end].to_string()
        });
        Ok(Self {
            revision: record.revision()?,
            status,
            finished_at: if status.is_active() {
                None
            } else {
                Some(chrono::Utc::now().timestamp())
            },
            error,
        })
    }
}
