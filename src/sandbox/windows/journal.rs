use super::{acl, identity::wide};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use windows_sys::Win32::{
    Foundation::LocalFree,
    Security::{Authorization::ConvertStringSidToSidW, Isolation::DeleteAppContainerProfile},
};

#[derive(Serialize, Deserialize)]
struct Record {
    sid: String,
    profile: String,
    paths: Vec<PathBuf>,
}

/// 【Windows沙箱】【授权日志】文件锁表示活动执行，写前日志支持进程异常退出后重试清理。
pub(super) struct Journal {
    file: Option<File>,
    path: PathBuf,
    record: Record,
}

/// 【Windows沙箱】【日志目录】返回可信用户本地目录；无参数，返回授权日志存储位置。
pub(super) fn root() -> Result<PathBuf> {
    Ok(
        PathBuf::from(std::env::var_os("LOCALAPPDATA").context("LOCALAPPDATA is unavailable")?)
            .join("sai/sandbox-grants"),
    )
}
impl Journal {
    /// 【Windows沙箱】【登记身份】持有活动锁并同步身份；参数为 SID 和档案名，返回日志守卫。
    pub(super) fn new(sid: String, profile: String) -> Result<Self> {
        std::fs::create_dir_all(root()?)?;
        let path = root()?.join(format!("{}.pending", uuid::Uuid::new_v4()));
        let file = File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)?;
        file.lock()?;
        let mut this = Self {
            file: Some(file),
            path,
            record: Record {
                sid,
                profile,
                paths: Vec::new(),
            },
        };
        let mut bytes = serde_json::to_vec(&this.record)?;
        bytes.push(b'\n');
        this.append(&bytes)?;
        let committed = this.path.with_extension("jsonl");
        std::fs::rename(&this.path, &committed)?;
        this.path = committed;
        Ok(this)
    }
    /// 【Windows沙箱】【写前登记】在修改 ACL 前同步路径；参数为目标路径，返回持久化结果。
    pub(super) fn register(&mut self, path: &Path) -> Result<()> {
        if !self.record.paths.iter().any(|entry| entry == path) {
            let mut bytes = serde_json::to_vec(path)?;
            bytes.push(b'\n');
            self.append(&bytes)?;
            self.record.paths.push(path.into());
        }
        Ok(())
    }
    /// 【Windows沙箱】【同步日志】追加独立完整记录，崩溃时保留此前全部授权路径。
    /// 参数：含换行的记录字节；返回刷盘结果。
    fn append(&mut self, bytes: &[u8]) -> Result<()> {
        let file = self.file.as_mut().context("closed grant journal")?;
        file.write_all(bytes)?;
        file.sync_all()?;
        Ok(())
    }
    /// 【Windows沙箱】【结束日志】全部授权回收后删除日志；无参数，返回清理结果。
    pub(super) fn complete(&mut self) -> Result<()> {
        std::fs::remove_file(&self.path)?;
        drop(self.file.take());
        Ok(())
    }
}

/// 【Windows沙箱】【崩溃恢复】只回收未持锁身份的专属 ACE，不覆盖其他主体权限。
/// 参数：无；返回回收结果，损坏日志或权限错误阻止后续沙箱启动。
pub(super) fn recover() -> Result<()> {
    let root = root()?;
    if !root.exists() {
        return Ok(());
    }
    for item in std::fs::read_dir(&root)? {
        let path = item?.path();
        if path
            .extension()
            .is_none_or(|ext| ext != "jsonl" && ext != "json")
        {
            continue;
        }
        let mut file = match File::options().read(true).write(true).open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        match file.try_lock() {
            Ok(()) => {}
            Err(std::fs::TryLockError::WouldBlock) => continue,
            Err(error) => anyhow::bail!("lock grant journal: {error}"),
        }
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        let mut lines = bytes.split_inclusive(|byte| *byte == b'\n');
        let first = lines.next().context("empty sandbox grant journal")?;
        let mut record: Record =
            serde_json::from_slice(first).context("invalid sandbox grant journal")?;
        for line in lines {
            // 1. 【Windows沙箱】【残行恢复】未刷完的最后路径尚未修改 ACL，可以忽略
            if line.last() == Some(&b'\n') {
                record
                    .paths
                    .push(serde_json::from_slice(line).context("invalid sandbox grant path")?);
            }
        }
        anyhow::ensure!(
            record.profile.starts_with("sai.sandbox."),
            "invalid sandbox profile"
        );
        let mut sid = std::ptr::null_mut();
        anyhow::ensure!(
            unsafe { ConvertStringSidToSidW(wide(&record.sid).as_ptr(), &mut sid) } != 0,
            "invalid grant SID"
        );
        let result = record.paths.iter().rev().try_for_each(|path| {
            if path.exists() {
                acl::revoke(path, sid)
            } else {
                Ok(())
            }
        });
        unsafe {
            LocalFree(sid);
        }
        result?;
        let code = unsafe { DeleteAppContainerProfile(wide(&record.profile).as_ptr()) };
        anyhow::ensure!(
            code >= 0 || code as u32 == 0x80070002,
            "delete stale AppContainer: {code:#x}"
        );
        std::fs::remove_file(path)?;
        drop(file);
    }
    Ok(())
}
