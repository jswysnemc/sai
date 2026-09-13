use super::record::{Flag, LegacyRecord};
use crate::{paths::SaiPaths, plugins::private::paths};
use anyhow::{ensure, Context, Result};
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use sai_plugin_runtime::host::{validate_scheduled_id, ScheduledStatus};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Write},
};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Flags {
    version: u32,
    records: BTreeMap<String, Flag>,
}

pub(super) struct Store {
    state: Dir,
    directory: Dir,
}

impl Store {
    /// 【旧闹钟兼容】【目录归属】旧文件只读，兼容状态使用单独的插件命名空间。
    /// @param paths 可信应用路径
    /// @returns 两个独立目录句柄
    pub fn open(paths: &SaiPaths) -> Result<Self> {
        let (directory, _) = paths::namespace(&paths.state_dir, "plugin-legacy", "alarm", "")?;
        let (state, _) = paths::root(&paths.state_dir)?;
        Ok(Self { state, directory })
    }

    /// 【旧闹钟兼容】【原版快照】有界读取原子发布的旧文件，拒绝链接、管道和重复身份。
    /// @returns 最多 128 条旧任务，不修改原文件
    pub fn records(&self) -> Result<Vec<LegacyRecord>> {
        let Some(bytes) = read(&self.state, "alarms.json", 4 * 1024 * 1024)? else {
            return Ok(Vec::new());
        };
        if std::str::from_utf8(&bytes)?.trim().is_empty() {
            return Ok(Vec::new());
        }
        let records: Vec<LegacyRecord> =
            serde_json::from_slice(&bytes).context("decode legacy alarms.json")?;
        ensure!(records.len() <= 128, "legacy alarm count exceeds 128");
        let mut ids = BTreeSet::new();
        for record in &records {
            record.validate()?;
            ensure!(ids.insert(&record.id), "duplicate legacy alarm id");
        }
        Ok(records)
    }

    /// 【旧闹钟兼容】【状态快照】校验版本、大小和每条通用状态。
    /// @returns 独立状态表
    fn flags(&self) -> Result<Flags> {
        let Some(bytes) = read(&self.directory, "states.json", 1024 * 1024)? else {
            return Ok(Flags {
                version: 1,
                records: BTreeMap::new(),
            });
        };
        let flags: Flags = serde_json::from_slice(&bytes).context("decode legacy alarm states")?;
        ensure!(
            flags.version == 1 && flags.records.len() <= 128,
            "invalid legacy alarm state version or count"
        );
        for (id, flag) in &flags.records {
            validate_scheduled_id(id)?;
            ensure!(
                flag.revision.len() == 64
                    && flag.revision.bytes().all(|byte| byte.is_ascii_hexdigit()),
                "invalid legacy alarm state identity"
            );
            ensure!(
                flag.error.as_ref().is_none_or(|error| error.len() <= 4096)
                    && flag.status.is_active() == flag.finished_at.is_none(),
                "invalid legacy alarm state"
            );
            ensure!(
                flag.finished_at
                    .is_none_or(|time| (0..=253402300799).contains(&time)),
                "invalid legacy alarm state timestamp"
            );
        }
        Ok(flags)
    }

    /// 【旧闹钟兼容】【读取状态】旧记录内容变化后不继承之前的状态。
    /// @param record 旧任务
    /// @returns 匹配同一内容身份的状态
    pub fn flag(&self, record: &LegacyRecord) -> Result<Option<Flag>> {
        let revision = record.revision()?;
        Ok(self
            .flags()?
            .records
            .remove(&record.task_id())
            .filter(|flag| flag.revision == revision))
    }

    /// 【旧闹钟兼容】【组合快照】状态表只读取一次，旧记录变化时不沿用旧身份状态。
    /// @returns 旧记录及匹配状态
    pub fn snapshot(&self) -> Result<Vec<(LegacyRecord, Option<Flag>)>> {
        let records = self.records()?;
        let mut flags = self.flags()?;
        records
            .into_iter()
            .map(|record| {
                let revision = record.revision()?;
                let flag = flags
                    .records
                    .remove(&record.task_id())
                    .filter(|flag| flag.revision == revision);
                Ok((record, flag))
            })
            .collect()
    }

    /// 【旧闹钟兼容】【取消互斥】串行取消防止一个失败请求覆盖另一个成功请求。
    /// @returns 有界兼容范围内的取消锁，占用时提示重试
    pub fn cancellation_lock(&self) -> Result<paths::Lock> {
        paths::lock(&self.directory, ".cancellations.lock")
    }

    /// 【旧闹钟兼容】【执行互斥】新程序承接旧工作入口时独占该任务，结束时释放。
    /// @param record 已校验任务
    /// @returns 执行锁
    pub fn worker_lock(&self, record: &LegacyRecord) -> Result<paths::Lock> {
        let _lock = paths::lock(&self.directory, ".operations.lock")?;
        let records = self.records()?;
        let current = records
            .iter()
            .find(|item| item.id == record.id)
            .context("legacy alarm disappeared")?;
        ensure!(
            current.revision()? == record.revision()?,
            "legacy alarm identity changed"
        );
        self.clean_locks(&records)?;
        paths::lock(
            &self.directory,
            &format!("{}.worker.lock", record.task_id()),
        )
    }

    /// 【旧闹钟兼容】【历史锁清理】只删除快照中缺失且无人持有的锁，创建执行锁时持有同一操作锁。
    /// @param records 当前旧记录
    /// @returns 清理结果，不删除任何活动执行锁
    fn clean_locks(&self, records: &[LegacyRecord]) -> Result<()> {
        let ids: BTreeSet<_> = records.iter().map(LegacyRecord::task_id).collect();
        for (index, entry) in self.directory.entries()?.take(265).enumerate() {
            ensure!(
                index < 264,
                "legacy alarm directory contains too many entries"
            );
            let name = entry?.file_name();
            let name = name.to_str().context("invalid legacy alarm filename")?;
            let Some(id) = name.strip_suffix(".worker.lock") else {
                continue;
            };
            validate_scheduled_id(id)?;
            if ids.contains(id) {
                continue;
            }
            match paths::lock(&self.directory, name) {
                Ok(_lease) => self.directory.remove_file(name)?,
                Err(error) if is_busy(&error) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    /// 【旧闹钟兼容】【条件转换】原子检查当前状态，清理已不在旧快照中的状态后写入新值。
    /// @param record 任务；next 为新状态或删除；allowed 为可转换的当前状态
    /// @returns 提交后状态，条件不符时保留当前值
    pub fn transition(
        &self,
        record: &LegacyRecord,
        next: Option<Flag>,
        allowed: &[ScheduledStatus],
    ) -> Result<Option<Flag>> {
        let _lock = paths::lock(&self.directory, ".operations.lock")?;
        let records = self.records()?;
        let current = records
            .iter()
            .find(|item| item.id == record.id)
            .context("legacy alarm disappeared")?;
        ensure!(
            current.revision()? == record.revision()?,
            "legacy alarm identity changed"
        );
        let ids: BTreeSet<_> = records.iter().map(LegacyRecord::task_id).collect();
        let mut flags = self.flags()?;
        flags.records.retain(|id, _| ids.contains(id));
        let id = record.task_id();
        let revision = record.revision()?;
        let old = flags
            .records
            .get(&id)
            .filter(|flag| flag.revision == revision)
            .cloned();
        if !allowed.contains(
            &old.as_ref()
                .map(|flag| flag.status)
                .unwrap_or(current.status()),
        ) {
            return Ok(old);
        }
        match &next {
            Some(flag) => {
                flags.records.insert(id, flag.clone());
            }
            None => {
                flags.records.remove(&id);
            }
        }
        self.write(&flags)?;
        Ok(next)
    }

    /// 【旧闹钟兼容】【原子保存】同步临时文件后替换状态，不触碰原版任务文件。
    /// @param flags 有界状态表
    /// @returns 保存结果
    fn write(&self, flags: &Flags) -> Result<()> {
        match self.directory.symlink_metadata("states.json") {
            Ok(metadata) => ensure!(
                metadata.is_file() && !metadata.file_type().is_symlink(),
                "legacy alarm states require a regular file"
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let bytes = serde_json::to_vec(flags)?;
        ensure!(
            bytes.len() <= 1024 * 1024,
            "legacy alarm state exceeds 1 MiB"
        );
        let temporary = format!(".{}.tmp", uuid::Uuid::new_v4().simple());
        let result = (|| {
            let mut file = self
                .directory
                .open_with(&temporary, OpenOptions::new().write(true).create_new(true))?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            self.directory
                .rename(&temporary, &self.directory, "states.json")?;
            Ok(())
        })();
        if result.is_err() {
            let _ = self.directory.remove_file(&temporary);
        }
        result
    }
}

/// 【旧闹钟兼容】【锁竞争】只把明确的文件锁占用识别为可重试状态。
/// @param error 操作错误
/// @returns 是否为立即失败的锁竞争
pub(super) fn is_busy(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        matches!(
            cause.downcast_ref::<std::fs::TryLockError>(),
            Some(std::fs::TryLockError::WouldBlock)
        )
    })
}

/// 【旧闹钟兼容】【文件读取】不跟随链接，Unix 非阻塞打开后确认普通文件。
/// @param directory 目录；name 为固定文件名；limit 为字节上限
/// @returns 完整有界字节，缺失时返回 None
fn read(directory: &Dir, name: &str, limit: usize) -> Result<Option<Vec<u8>>> {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    #[cfg(unix)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = match directory.open_with(name, &options) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    ensure!(
        file.metadata()?.is_file(),
        "legacy alarm data requires a regular file"
    );
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= limit, "legacy alarm file exceeds byte limit");
    Ok(Some(bytes))
}
