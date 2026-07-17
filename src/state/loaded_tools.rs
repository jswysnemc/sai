use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Debug, Default, Deserialize, Serialize)]
struct LoadedToolsState {
    loaded_tools: Vec<String>,
}

/// 读取当前会话已经载入的工具名称。
///
/// 参数:
/// - `path`: 已载入工具状态文件路径
///
/// 返回:
/// - 去重和排序后的工具名称列表
pub(super) fn load(path: &Path) -> Result<Vec<String>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read loaded tools state: {}", path.display()))?;
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }
    let state = serde_json::from_str::<LoadedToolsState>(&raw)
        .with_context(|| format!("failed to parse loaded tools state: {}", path.display()))?;
    Ok(normalize_tool_names(&state.loaded_tools))
}

/// 保存当前会话已经载入的工具名称。
///
/// 参数:
/// - `path`: 已载入工具状态文件路径
/// - `names`: 已载入工具名称列表
///
/// 返回:
/// - 保存是否成功
pub(super) fn save(path: &Path, names: &[String]) -> Result<()> {
    let names = normalize_tool_names(names);
    if names.is_empty() {
        return clear(path);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create state dir: {}", parent.display()))?;
    }
    let state = LoadedToolsState {
        loaded_tools: names,
    };
    let raw = format!("{}\n", serde_json::to_string_pretty(&state)?);
    std::fs::write(path, raw)
        .with_context(|| format!("failed to write loaded tools state: {}", path.display()))
}

/// 清空当前会话已经载入的工具名称。
///
/// 参数:
/// - `path`: 已载入工具状态文件路径
///
/// 返回:
/// - 清空是否成功
pub(super) fn clear(path: &Path) -> Result<()> {
    if path.exists() {
        std::fs::remove_file(path)
            .with_context(|| format!("failed to remove loaded tools state: {}", path.display()))?;
    }
    Ok(())
}

/// 规范化工具名称列表。
///
/// 参数:
/// - `names`: 原始工具名称列表
///
/// 返回:
/// - 去空、去重和排序后的工具名称列表
fn normalize_tool_names(names: &[String]) -> Vec<String> {
    names
        .iter()
        .map(|name| name.trim())
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_and_loads_unique_tool_names() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("loaded-tools.json");

        save(
            &path,
            &[
                "web_search".to_string(),
                " ".to_string(),
                "web_search".to_string(),
                "web_fetch".to_string(),
            ],
        )
        .unwrap();

        assert_eq!(
            load(&path).unwrap(),
            vec!["web_fetch".to_string(), "web_search".to_string()]
        );
    }

    #[test]
    fn clears_empty_tool_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("loaded-tools.json");
        save(&path, &["web_search".to_string()]).unwrap();

        save(&path, &[]).unwrap();

        assert!(!path.exists());
    }
}
