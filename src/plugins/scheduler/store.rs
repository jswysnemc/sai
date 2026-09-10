use super::record::JobRecord;
use crate::plugins::private::paths;
use anyhow::{bail, ensure, Context, Result};
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use sai_plugin_runtime::host::{validate_scheduled_id, MAX_ACTIVE_TASKS, MAX_SCHEDULED_TASKS};
use std::io::{Read, Write};
use std::path::Path;

const MAX_RECORD_BYTES: usize = 256 * 1024;

/// 【插件调度】【目录边界】所有记录和锁均位于宿主绑定的插件命名空间。
pub(super) struct Store {
    pub directory: Dir,
    plugin: String,
}

impl Store {
    /// 【插件调度】【打开目录】复用不跟随符号链接的私有目录句柄。
    /// @param state_dir 宿主状态根；plugin 为已校验插件标识
    /// @returns 独立任务存储
    pub fn open(state_dir: &Path, plugin: &str) -> Result<Self> {
        let (directory, _) = paths::namespace(state_dir, "plugin-jobs", plugin, "")?;
        Ok(Self {
            directory,
            plugin: plugin.into(),
        })
    }

    /// 【插件调度】【事务锁】所有创建和状态转换使用同一个短锁。
    /// @returns 操作守卫；并发占用返回明确的重试错误
    pub fn lock(&self) -> Result<paths::Lock> {
        paths::lock(&self.directory, ".operations.lock")
    }

    /// 【插件调度】【读取快照】原子替换保证读者取得完整版本，禁止链接和特殊文件。
    /// @param id 任务标识
    /// @returns 已校验记录，缺失返回 None
    pub fn get(&self, id: &str) -> Result<Option<JobRecord>> {
        validate_scheduled_id(id)?;
        let file = match self
            .directory
            .open_with(format!("{id}.json"), &read_options())
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        ensure!(
            file.metadata()?.is_file(),
            "scheduled record requires a regular file"
        );
        let mut bytes = Vec::new();
        file.take(MAX_RECORD_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() <= MAX_RECORD_BYTES,
            "scheduled record exceeds 256 KiB"
        );
        let record: JobRecord =
            serde_json::from_slice(&bytes).context("decode scheduled record")?;
        record.validate(&self.plugin, id)?;
        Ok(Some(record))
    }

    /// 【插件调度】【任务列表】只读取本插件的有界记录，查询不启动或恢复任务。
    /// @returns 按创建时间和标识排序的记录
    pub fn list(&self) -> Result<Vec<JobRecord>> {
        let mut records = Vec::new();
        for (index, entry) in self
            .directory
            .entries()?
            .take(2 * MAX_SCHEDULED_TASKS + 9)
            .enumerate()
        {
            ensure!(
                index < 2 * MAX_SCHEDULED_TASKS + 8,
                "scheduled directory contains too many entries"
            );
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_str().context("invalid scheduled record filename")?;
            if let Some(id) = name.strip_suffix(".json") {
                if let Some(record) = self.get(id)? {
                    records.push(record);
                }
                ensure!(
                    records.len() <= MAX_SCHEDULED_TASKS,
                    "scheduled record count exceeds 128"
                );
            }
        }
        records
            .sort_by(|a, b| (a.task.created_at, &a.task.id).cmp(&(b.task.created_at, &b.task.id)));
        Ok(records)
    }

    /// 【插件调度】【创建配额】在调用者持有事务锁时清理最旧终态记录，活动任务不能被驱逐。
    /// @returns 可以创建一个任务时成功
    pub fn reserve(&self) -> Result<()> {
        let records = self.list()?;
        ensure!(
            records
                .iter()
                .filter(|record| record.task.status.is_active())
                .count()
                < MAX_ACTIVE_TASKS,
            "plugin exceeds 32 active scheduled tasks"
        );
        if records.len() >= MAX_SCHEDULED_TASKS {
            let oldest = records
                .iter()
                .find(|record| !record.task.status.is_active())
                .context("scheduled task history is full")?;
            self.remove(&oldest.task.id)?;
        }
        Ok(())
    }

    /// 【插件调度】【原子保存】调用者持有事务锁，写入同步后才替换公开文件。
    /// @param record 完整记录
    /// @returns 保存结果
    pub fn write(&self, record: &JobRecord) -> Result<()> {
        record.validate(&self.plugin, &record.task.id)?;
        let bytes = serde_json::to_vec(record)?;
        ensure!(
            bytes.len() <= MAX_RECORD_BYTES,
            "scheduled record exceeds 256 KiB"
        );
        let name = format!("{}.json", record.task.id);
        match self.directory.symlink_metadata(&name) {
            Ok(metadata) => ensure!(
                metadata.is_file() && !metadata.file_type().is_symlink(),
                "scheduled record requires a regular file"
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let temporary = format!(".{}.tmp", uuid::Uuid::new_v4().simple());
        let result = (|| {
            let mut file = self
                .directory
                .open_with(&temporary, OpenOptions::new().write(true).create_new(true))?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            self.directory.rename(&temporary, &self.directory, &name)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = self.directory.remove_file(&temporary);
        }
        result
    }

    /// 【插件调度】【历史清理】只由持锁配额清理调用，随机标识不会重新分配。
    /// @param id 已完成任务标识
    /// @returns 删除结果
    fn remove(&self, id: &str) -> Result<()> {
        validate_scheduled_id(id)?;
        self.directory.remove_file(format!("{id}.json"))?;
        match self.directory.remove_file(format!("{id}.lock")) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => bail!(error),
        }
    }
}

/// 【插件调度】【读取选项】末级不跟随链接，Unix 管道不能阻塞读取线程。
/// @returns 只读打开选项
pub(super) fn read_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    #[cfg(unix)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    options
}
