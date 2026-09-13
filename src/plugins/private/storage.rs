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
    transaction(paths, "plugin-state", id, session, request)
}

/// 【插件状态】【持久操作】独立授权插件记录，非法权限和请求在创建目录之前拒绝。
/// @param paths 应用路径；id 为绑定插件；request 为操作；capabilities 为授权；allow_writes 为可信权限
/// @returns 跨会话记录值或比较交换结果
pub(super) fn execute_plugin(
    paths: &SaiPaths,
    id: &str,
    request: StorageRequest,
    capabilities: &Capabilities,
    allow_writes: bool,
) -> Result<Value> {
    // 1. 【插件状态】【权限复核】会话授权不能替代插件授权，删除和失败比较同样属于写入操作
    if !capabilities.system.plugin_storage {
        bail!("plugin storage is not allowed");
    }
    if !matches!(request, StorageRequest::Get { .. }) && !allow_writes {
        bail!("read-only plugin callback cannot mutate plugin storage");
    }
    transaction(paths, "plugin-storage", id, "", request)
}

/// 【插件状态】【共用事务】在独立类别锁下执行读取、比较和原子替换，作用域只能由宿主指定。
/// @param paths 应用路径；category 为内部类别；id 为插件；session 为作用域；request 为操作
/// @returns 记录值或比较交换结果
fn transaction(
    paths: &SaiPaths,
    category: &str,
    id: &str,
    session: &str,
    request: StorageRequest,
) -> Result<Value> {
    // 【插件状态】【记录预算】会话与持久记录均校验比较值，失败比较不能绕过上限
    match &request {
        StorageRequest::Set { value, .. } => validate_value(value)?,
        StorageRequest::CompareExchange {
            expected, value, ..
        } => {
            validate_value(expected)?;
            validate_value(value)?;
        }
        StorageRequest::Get { .. } => {}
    }
    let key = match &request {
        StorageRequest::Get { key }
        | StorageRequest::Set { key, .. }
        | StorageRequest::CompareExchange { key, .. } => key,
    };
    validate_storage_key(key)?;
    let name = format!("{}.json", paths::hash(key));
    let (root, _) = paths::root(&paths.state_dir)?;
    let _lock = paths::lock(&root, &format!(".{category}.lock"))?;
    let (directory, _) = paths::namespace(&paths.state_dir, category, id, session)?;
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
/// @param directory 私有记录目录；name 为摘要文件名
/// @returns 已解析记录，缺失返回 null
fn read(directory: &Dir, name: &str) -> Result<Value> {
    Ok(read_optional(directory, name)?.unwrap_or(Value::Null))
}

/// 【插件状态】【记录存在性】区分缺失文件和已保存 null，供旧会话接续保留删除事实
/// @param directory 私有记录目录；name 为固定文件名
/// @returns 完整记录，文件缺失时为 None
pub(in crate::plugins) fn read_optional(directory: &Dir, name: &str) -> Result<Option<Value>> {
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
    if !file.metadata()?.is_file() {
        bail!("plugin storage requires a regular file");
    }
    let mut bytes = Vec::new();
    file.take(MAX_VALUE as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_VALUE {
        bail!("plugin storage value exceeds 256 KiB");
    }
    serde_json::from_slice(&bytes)
        .map(Some)
        .context("decode plugin storage record")
}

/// 【插件状态】【值校验】使用 JSON 序列化字节数计算配额，包含引号和转义开销。
/// @param value 要保存或比较的记录值
/// @returns 大小未超过单条上限时成功
fn validate_value(value: &Value) -> Result<()> {
    if serde_json::to_vec(value)?.len() > MAX_VALUE {
        bail!("plugin storage value exceeds 256 KiB");
    }
    Ok(())
}

/// 【插件状态】【原子保存】写入新文件并同步后重命名，失败时删除未发布文件。
/// @param directory 私有记录目录；name 为摘要文件名；value 为新值，null 表示删除
/// @returns 保存结果
fn write(directory: &Dir, name: &str, value: &Value) -> Result<()> {
    if value.is_null() {
        return match directory.remove_file(name) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        };
    }
    validate_value(value)?;
    if !directory.try_exists(name)? && directory.entries()?.take(128).count() >= 128 {
        bail!("plugin storage exceeds 128 keys");
    }
    write_document(directory, name, value)
}

/// 【插件状态】【文档发布】原子保存完整 JSON，null 同样保留为显式记录
/// @param directory 已验证目录；name 为固定记录名；value 为有界 JSON
/// @returns 同步和原子替换结果；失败清理临时文件
pub(in crate::plugins) fn write_document(directory: &Dir, name: &str, value: &Value) -> Result<()> {
    let bytes = serde_json::to_vec(value)?;
    if bytes.len() > MAX_VALUE {
        bail!("plugin storage value exceeds 256 KiB");
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
