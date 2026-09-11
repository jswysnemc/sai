use super::RuntimeOverrides;
use crate::{
    paths::SaiPaths,
    plugins::private::{paths, storage},
};
use anyhow::{bail, Context, Result};
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use sai_plugin_runtime::{host::StorageRequest, Capabilities};
use serde_json::{json, Value};
use std::{
    io::Read,
    path::{Component, Path},
};

const RECORD: &str = "todos.plugin.json";
const MAX_BYTES: usize = 256 * 1024;

/// 【待办兼容】【语言设置】只有内置包继承界面语言，显式插件设置优先
/// @param settings 显式设置；declared 为原能力声明
/// @returns 不扩大文件授权的运行时设置
pub(super) fn resolve(settings: &Value, declared: &Capabilities) -> Result<RuntimeOverrides> {
    let mut merged = json!({"language": if crate::i18n::is_zh() { "zh" } else { "en" }});
    merged.as_object_mut().unwrap().extend(
        settings
            .as_object()
            .context("todo settings must be an object")?
            .clone(),
    );
    Ok(RuntimeOverrides {
        settings: merged,
        capabilities: declared.clone(),
    })
}

/// 【待办兼容】【会话文件接续】固定私有键绑定原会话目录，清空对话时继续保留计划
/// @param app 应用目录；session 为可信会话目录；request 为私有操作；capabilities 为授权
/// @returns 兼容结果；普通直接入口及其他键返回 None
pub(in crate::plugins) fn storage_override(
    app: &SaiPaths,
    session: &str,
    request: &StorageRequest,
    capabilities: &Capabilities,
) -> Result<Option<Value>> {
    let key = match request {
        StorageRequest::Get { key }
        | StorageRequest::Set { key, .. }
        | StorageRequest::CompareExchange { key, .. } => key,
    };
    if key != "plan" || !Path::new(session).is_absolute() {
        return Ok(None);
    }
    if !capabilities.system.session_storage {
        bail!("plugin session storage is not allowed");
    }
    let relative = Path::new(session)
        .strip_prefix(&app.state_dir)
        .context("todo session must belong to application state")?;
    if relative
        .components()
        .any(|part| !matches!(part, Component::Normal(_)))
    {
        bail!("invalid todo session directory");
    }
    // 1. 【待办兼容】【目录归属】逐层使用目录句柄，不跟随目录或记录的符号链接
    let (mut directory, _) = paths::root(&app.state_dir)?;
    let _lock = paths::lock(&directory, ".plugin-state.lock")?;
    for component in relative.components() {
        directory = match directory.open_dir_nofollow(component.as_os_str()) {
            Ok(directory) => directory,
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound
                    && matches!(request, StorageRequest::Get { .. }) =>
            {
                return Ok(Some(Value::Null))
            }
            Err(error) => return Err(error).context("open bound todo session directory"),
        };
    }
    // 2. 【待办兼容】【旧记录只读】新文件存在即为唯一来源，包括显式 null 墓碑
    let previous = match storage::read_optional(&directory, RECORD)? {
        Some(value) => value,
        None => legacy(&directory)?,
    };
    let result = match request {
        StorageRequest::Get { .. } => previous,
        StorageRequest::Set { value, .. } => {
            storage::write_document(&directory, RECORD, value)?;
            value.clone()
        }
        StorageRequest::CompareExchange {
            expected, value, ..
        } => {
            if serde_json::to_vec(expected)?.len() > MAX_BYTES
                || serde_json::to_vec(value)?.len() > MAX_BYTES
            {
                bail!("plugin storage value exceeds 256 KiB");
            }
            if previous != *expected {
                return Ok(Some(Value::Bool(false)));
            }
            storage::write_document(&directory, RECORD, value)?;
            Value::Bool(true)
        }
    };
    Ok(Some(result))
}

/// 【待办兼容】【旧文件种子】只交付原始数组，状态规则与归档判断属于 Lua
/// @param directory 可信会话目录
/// @returns 原始旧数据；两份文件都不存在时为 null
fn legacy(directory: &Dir) -> Result<Value> {
    let items = read_legacy(directory, "todos.json")?;
    let history = read_legacy(directory, "todos.history.json")?;
    if items.is_none() && history.is_none() {
        return Ok(Value::Null);
    }
    let value = json!({"version":0,"items":items.unwrap_or_else(|| json!([])),"history":history.unwrap_or_else(|| json!([]))});
    if serde_json::to_vec(&value)?.len() > MAX_BYTES {
        bail!("legacy todo state exceeds 256 KiB");
    }
    Ok(value)
}

/// 【待办兼容】【有界旧读取】保留空白文件语义，拒绝损坏数据、链接及特殊对象
/// @param directory 可信目录；name 为固定旧文件名
/// @returns 完整 JSON，文件缺失时为 None
fn read_legacy(directory: &Dir, name: &str) -> Result<Option<Value>> {
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
        Err(error) => return Err(error).with_context(|| format!("read legacy todo file {name}")),
    };
    if !file.metadata()?.is_file() {
        bail!("legacy todo requires a regular file");
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_BYTES {
        bail!("legacy todo file exceeds 256 KiB");
    }
    let text = std::str::from_utf8(&bytes).context("legacy todo is not UTF-8")?;
    Ok(Some(if text.trim().is_empty() {
        json!([])
    } else {
        serde_json::from_str(text).with_context(|| format!("parse legacy todo file {name}"))?
    }))
}
