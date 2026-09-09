use super::paths;
use crate::paths::SaiPaths;
use anyhow::{bail, Context, Result};
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use sai_plugin_runtime::{
    host::{validate_storage_key, StorageRequest},
    Capabilities,
};
use serde_json::Value;
use std::io::{Read, Write};

const MAX_VALUE: usize = 256 * 1024;

/// 【插件状态】【原子操作】同一应用根目录共享短锁，比较与替换在一个临界区完成。
/// @param paths 应用路径；id 为绑定插件；session 为可信会话；request 为操作；capabilities 为授权
/// @returns 记录值或比较交换是否成功
pub(super) fn execute(
    paths: &SaiPaths,
    id: &str,
    session: &str,
    request: StorageRequest,
    capabilities: &Capabilities,
) -> Result<Value> {
    if !capabilities.system.session_storage {
        bail!("plugin session storage is not allowed");
    }
    let key = match &request {
        StorageRequest::Get { key }
        | StorageRequest::Set { key, .. }
        | StorageRequest::CompareExchange { key, .. } => key,
    };
    validate_storage_key(key)?;
    let name = format!("{}.json", paths::hash(key));
    let (root, _) = paths::root(&paths.state_dir)?;
    let _lock = paths::lock(&root, ".plugin-state.lock")?;
    let (directory, _) = paths::namespace(&paths.state_dir, "plugin-state", id, session)?;
    let previous = read(&directory, &name)?;
    match request {
        StorageRequest::Get { .. } => Ok(previous),
        StorageRequest::Set { value, .. } => {
            write(&directory, &name, &value)?;
            Ok(value)
        }
        StorageRequest::CompareExchange {
            expected, value, ..
        } => {
            if previous != expected {
                return Ok(Value::Bool(false));
            }
            write(&directory, &name, &value)?;
            Ok(Value::Bool(true))
        }
    }
}

/// 【插件状态】【有界读取】禁止特殊文件和符号链接进入 JSON 读取。
/// @param directory 会话目录；name 为摘要文件名
/// @returns 已解析记录，缺失返回 null
fn read(directory: &Dir, name: &str) -> Result<Value> {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    #[cfg(unix)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = match directory.open_with(name, &options) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Value::Null),
        Err(error) => return Err(error.into()),
    };
    if !file.metadata()?.is_file() {
        bail!("plugin storage requires a regular file");
    }
    let mut bytes = Vec::new();
    file.take(MAX_VALUE as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_VALUE {
        bail!("plugin storage value exceeds 256 KiB");
    }
    serde_json::from_slice(&bytes).context("decode plugin session storage")
}

/// 【插件状态】【原子保存】写入新文件并同步后重命名，失败时删除未发布文件。
/// @param directory 会话目录；name 为摘要文件名；value 为新值，null 表示删除
/// @returns 保存结果
fn write(directory: &Dir, name: &str, value: &Value) -> Result<()> {
    if value.is_null() {
        return match directory.remove_file(name) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        };
    }
    let bytes = serde_json::to_vec(value)?;
    if bytes.len() > MAX_VALUE {
        bail!("plugin storage value exceeds 256 KiB");
    }
    if !directory.try_exists(name)? && directory.entries()?.take(128).count() >= 128 {
        bail!("plugin session storage exceeds 128 keys");
    }
    let temporary = format!(".{}.tmp", uuid::Uuid::new_v4());
    let result = (|| {
        let mut file =
            directory.open_with(&temporary, OpenOptions::new().write(true).create_new(true))?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        directory.rename(&temporary, directory, name)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = directory.remove_file(&temporary);
    }
    result
}

/// 【插件状态】【会话清理】只撤销目标会话的各插件记录，其他会话继续保留状态。
/// @param base 应用状态根目录；session 为完整会话作用域
/// @returns 清理结果；未创建记录目录时成功
pub(crate) fn clear(base: &std::path::Path, session: &str) -> Result<()> {
    let (root, _) = paths::root(base)?;
    let _lock = paths::lock(&root, ".plugin-state.lock")?;
    use cap_fs_ext::DirExt;
    let plugins = match root.open_dir_nofollow("plugin-state") {
        Ok(directory) => directory,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    for entry in plugins.entries()? {
        let plugin = plugins.open_dir_nofollow(entry?.file_name())?;
        match plugin.remove_dir_all(paths::hash(session)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}
