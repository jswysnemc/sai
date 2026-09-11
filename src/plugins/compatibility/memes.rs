use super::RuntimeOverrides;
use crate::{config::AppConfig, paths::SaiPaths};
use anyhow::{Context, Result};
use sai_plugin_runtime::Capabilities;
use serde_json::{json, Value};

/// 【表情兼容】【定向设置】旧字段只交给内置表情包，文件权限由本次明确目录派生
/// @param config 主配置；paths 为应用目录；settings 为显式设置；declared 为包声明
/// @returns 合并后的私有设置和目录能力，不修改持久配置或已有授权
pub(super) fn resolve(
    config: &AppConfig,
    paths: &SaiPaths,
    settings: &Value,
    declared: &Capabilities,
) -> Result<RuntimeOverrides> {
    let mut merged = serde_json::to_value(&config.plugins.memes)?;
    let defaults = merged
        .as_object_mut()
        .context("memes defaults must be an object")?;
    defaults.remove("enabled");
    defaults
        .get_mut("libraries")
        .and_then(Value::as_object_mut)
        .context("memes libraries must be an object")?
        .entry("default")
        .or_insert_with(|| json!(config.plugins.memes.default_library()));
    let builtin_dirs = match std::env::var_os("SAI_MEMES_DIR") {
        Some(path) => json!([path.to_str().context("SAI_MEMES_DIR must be UTF-8")?]),
        None => json!(["src/memes", "/usr/share/sai/memes"]),
    };
    defaults.extend(serde_json::from_value::<serde_json::Map<String, Value>>(
        json!({
            "builtin_dirs":builtin_dirs, "user_dir":paths.data_dir.join("memes"),
            "state_dir":paths.state_dir.join("memes"), "input_paths":["."],
            "language":if crate::i18n::is_zh() { "zh" } else { "en" },
        }),
    )?);
    defaults.extend(
        settings
            .as_object()
            .context("memes settings must be an object")?
            .clone(),
    );
    // 1. 【表情兼容】【最小目录】读取包含输入和索引，写入只包含用户库及发送记录
    let user = directory(&merged, "user_dir")?;
    let state = directory(&merged, "state_dir")?;
    let mut capabilities = declared.clone();
    capabilities.system.read_paths = directories(&merged, "input_paths")?
        .into_iter()
        .chain(directories(&merged, "builtin_dirs")?)
        .chain([user.clone(), state.clone()])
        .collect();
    capabilities.binary.write_paths = [user.clone(), state].into();
    capabilities.system.remove_paths = [user.clone()].into();
    capabilities.system.trash_paths = [user].into();
    capabilities.validate()?;
    Ok(RuntimeOverrides {
        settings: merged,
        capabilities,
    })
}

/// 【表情兼容】【单目录字段】拒绝 null、错误类型和空目录
/// @param settings 合并设置；key 为目录字段
/// @returns 目录字符串，进一步范围检查由能力契约执行
fn directory(settings: &Value, key: &str) -> Result<String> {
    settings
        .get(key)
        .and_then(Value::as_str)
        .filter(|path| !path.is_empty())
        .map(str::to_string)
        .with_context(|| format!("memes.{key} must be a nonempty directory"))
}

/// 【表情兼容】【目录列表】显式输入和内置目录分别受数量及类型限制
/// @param settings 合并设置；key 为列表字段
/// @returns 保留顺序的目录列表
fn directories(settings: &Value, key: &str) -> Result<Vec<String>> {
    let paths = settings
        .get(key)
        .and_then(Value::as_array)
        .context("memes directories must be an array")?;
    anyhow::ensure!(paths.len() <= 32, "memes directory list exceeds 32 entries");
    paths
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|path| !path.is_empty())
                .map(str::to_string)
                .context("memes directories must contain nonempty strings")
        })
        .collect()
}
