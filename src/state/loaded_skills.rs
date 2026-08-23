use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Debug, Default, Deserialize, Serialize)]
struct LoadedSkillsState {
    loaded_skills: Vec<String>,
}

/// 读取当前会话已经 load 过的 skill 名称。
///
/// 参数:
/// - `path`: 状态文件路径
///
/// 返回:
/// - 去重后的名称列表
pub(super) fn load(path: &Path) -> Result<Vec<String>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read loaded skills state: {}", path.display()))?;
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }
    let state = serde_json::from_str::<LoadedSkillsState>(&raw)
        .with_context(|| format!("failed to parse loaded skills state: {}", path.display()))?;
    Ok(normalize_names(&state.loaded_skills))
}

/// 保存当前会话已经 load 过的 skill 名称。
///
/// 参数:
/// - `path`: 状态文件路径
/// - `names`: 已加载名称
///
/// 返回:
/// - 保存是否成功
pub(super) fn save(path: &Path, names: &[String]) -> Result<()> {
    let names = normalize_names(names);
    if names.is_empty() {
        return clear(path);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create state dir: {}", parent.display()))?;
    }
    let state = LoadedSkillsState {
        loaded_skills: names,
    };
    let raw = format!("{}\n", serde_json::to_string_pretty(&state)?);
    std::fs::write(path, raw)
        .with_context(|| format!("failed to write loaded skills state: {}", path.display()))
}

/// 清空当前会话已经 load 过的 skill 名称。
///
/// 参数:
/// - `path`: 状态文件路径
///
/// 返回:
/// - 清空是否成功
pub(super) fn clear(path: &Path) -> Result<()> {
    if path.exists() {
        std::fs::remove_file(path)
            .with_context(|| format!("failed to remove loaded skills state: {}", path.display()))?;
    }
    Ok(())
}

/// 去掉空白并去重。
fn normalize_names(names: &[String]) -> Vec<String> {
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
    fn saves_and_loads_unique_skill_names() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("loaded-skills.json");
        save(
            &path,
            &[
                "gpu-passthrough".to_string(),
                " ".to_string(),
                "gpu-passthrough".to_string(),
                "drawio".to_string(),
            ],
        )
        .unwrap();
        assert_eq!(
            load(&path).unwrap(),
            vec!["drawio".to_string(), "gpu-passthrough".to_string()]
        );
    }

    #[test]
    fn clears_empty_skill_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("loaded-skills.json");
        save(&path, &["gpu-passthrough".to_string()]).unwrap();
        save(&path, &[]).unwrap();
        assert!(!path.exists());
    }
}
