use crate::config::AppConfig;
use crate::paths::SaiPaths;
use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::path::PathBuf;

/// 设置页使用的 Skill 管理条目。
#[derive(Clone, Serialize)]
pub(crate) struct ManagedSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub scope: String,
    pub directory_name: String,
    pub path: String,
    pub enabled: bool,
}

/// 扫描全局与当前人格 Skill 目录，包含已禁用条目。
///
/// 参数:
/// - `config`: 当前应用配置
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 可管理 Skill 列表
pub(crate) fn list_managed_skills(
    config: &AppConfig,
    paths: &SaiPaths,
) -> Result<Vec<ManagedSkill>> {
    let mut skills = Vec::new();
    for (scope, root) in skill_roots(config, paths) {
        if !root.exists() {
            continue;
        }
        for entry in std::fs::read_dir(&root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let directory_name = entry.file_name().to_string_lossy().to_string();
            let file = entry.path().join("SKILL.md");
            if !file.is_file() {
                continue;
            }
            let raw = std::fs::read_to_string(&file)?;
            let name = frontmatter_value(&raw, "name").unwrap_or_else(|| directory_name.clone());
            let description = frontmatter_value(&raw, "description").unwrap_or_default();
            skills.push(ManagedSkill {
                id: format!("{scope}:{directory_name}"),
                name,
                description,
                scope: scope.to_string(),
                directory_name,
                path: file.display().to_string(),
                enabled: !entry.path().join(".disabled").exists(),
            });
        }
    }
    skills.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then(left.scope.cmp(&right.scope))
    });
    Ok(skills)
}

/// 读取指定 Skill 的原始 SKILL.md。
///
/// 参数:
/// - `id`: 管理条目标识，格式为 scope:directory
/// - `config`: 当前应用配置
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - Skill 元数据与原始内容
pub(crate) fn read_managed_skill(
    id: &str,
    config: &AppConfig,
    paths: &SaiPaths,
) -> Result<(ManagedSkill, String)> {
    let skill = find_skill(id, config, paths)?;
    let content = std::fs::read_to_string(&skill.path)
        .with_context(|| format!("failed to read skill: {}", skill.path))?;
    Ok((skill, content))
}

/// 在全局 Skills 目录新增 Skill。
///
/// 参数:
/// - `directory_name`: 新目录名称
/// - `content`: SKILL.md 原始内容
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 新建完成后的管理标识
pub(crate) fn create_managed_skill(
    directory_name: &str,
    content: &str,
    paths: &SaiPaths,
) -> Result<String> {
    validate_directory_name(directory_name)?;
    validate_skill_content(content)?;
    let directory = paths.skills_dir.join(directory_name);
    if directory.exists() {
        bail!("skill directory already exists: {directory_name}");
    }
    std::fs::create_dir_all(&directory)?;
    std::fs::write(directory.join("SKILL.md"), content)?;
    Ok(format!("global:{directory_name}"))
}

/// 更新指定 Skill 的 SKILL.md 内容。
///
/// 参数:
/// - `id`: 管理条目标识
/// - `content`: 新的完整文档
/// - `config`: 当前应用配置
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 写入结果
pub(crate) fn update_managed_skill(
    id: &str,
    content: &str,
    config: &AppConfig,
    paths: &SaiPaths,
) -> Result<()> {
    validate_skill_content(content)?;
    let file = resolve_skill_file(id, config, paths)?;
    std::fs::write(file, content)?;
    Ok(())
}

/// 设置指定 Skill 的启用状态。
///
/// 参数:
/// - `id`: 管理条目标识
/// - `enabled`: 是否启用
/// - `config`: 当前应用配置
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 标记文件更新结果
pub(crate) fn set_managed_skill_enabled(
    id: &str,
    enabled: bool,
    config: &AppConfig,
    paths: &SaiPaths,
) -> Result<()> {
    let directory = resolve_skill_directory(id, config, paths)?;
    let marker = directory.join(".disabled");
    if enabled {
        if marker.exists() {
            std::fs::remove_file(marker)?;
        }
    } else {
        std::fs::write(marker, "")?;
    }
    Ok(())
}

/// 根据管理标识定位 Skill 条目。
fn find_skill(id: &str, config: &AppConfig, paths: &SaiPaths) -> Result<ManagedSkill> {
    list_managed_skills(config, paths)?
        .into_iter()
        .find(|skill| skill.id == id)
        .with_context(|| format!("skill not found: {id}"))
}

/// 根据管理标识解析 SKILL.md 路径。
fn resolve_skill_file(id: &str, config: &AppConfig, paths: &SaiPaths) -> Result<PathBuf> {
    let directory = resolve_skill_directory(id, config, paths)?;
    let file = directory.join("SKILL.md");
    if !file.is_file() {
        bail!("skill file not found: {id}");
    }
    Ok(file)
}

/// 根据管理标识解析受控 Skill 目录。
fn resolve_skill_directory(id: &str, config: &AppConfig, paths: &SaiPaths) -> Result<PathBuf> {
    let (scope, directory_name) = id.split_once(':').context("invalid skill id")?;
    validate_directory_name(directory_name)?;
    let root = skill_roots(config, paths)
        .into_iter()
        .find_map(|(candidate, root)| (candidate == scope).then_some(root))
        .context("invalid skill scope")?;
    let directory = root.join(directory_name);
    if !directory.is_dir() {
        bail!("skill directory not found: {id}");
    }
    Ok(directory)
}

/// 返回全局与当前人格 Skills 根目录。
fn skill_roots(config: &AppConfig, paths: &SaiPaths) -> Vec<(&'static str, PathBuf)> {
    let mut roots = vec![("global", paths.skills_dir.clone())];
    let persona = config.active_persona_skills_dir(paths);
    if persona != paths.skills_dir {
        roots.push(("persona", persona));
    }
    roots
}

/// 校验目录名称为单个安全路径片段。
fn validate_directory_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.starts_with('.')
        || !name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        bail!("skill directory name may only contain letters, numbers, hyphens, and underscores");
    }
    Ok(())
}

/// 校验 Skill 文档包含可用的 YAML frontmatter。
fn validate_skill_content(content: &str) -> Result<()> {
    if frontmatter_value(content, "name")
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        bail!("SKILL.md frontmatter requires a non-empty name");
    }
    if frontmatter_value(content, "description")
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        bail!("SKILL.md frontmatter requires a non-empty description");
    }
    Ok(())
}

/// 读取简单 YAML frontmatter 字段。
fn frontmatter_value(raw: &str, key: &str) -> Option<String> {
    let mut lines = raw.lines();
    if lines.next()? != "---" {
        return None;
    }
    for line in lines {
        if line.trim() == "---" {
            break;
        }
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.trim() == key {
            return Some(value.trim().trim_matches('"').to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// 构造 Skill 管理测试使用的路径。
    fn test_paths(root: &Path) -> SaiPaths {
        SaiPaths {
            config_dir: root.join("config"),
            config_file: root.join("config/config.jsonc"),
            secrets_file: root.join("config/secrets.jsonc"),
            skills_dir: root.join("config/skills"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            state_dir: root.join("state"),
            pictures_dir: root.join("pictures"),
            fish_hook_file: root.join("fish/sai.fish"),
            bash_hook_file: root.join("shell/bash-hook.sh"),
            zsh_hook_file: root.join("shell/zsh-hook.zsh"),
            powershell_hook_file: root.join("shell/powershell-hook.ps1"),
        }
    }

    /// 验证新增、编辑和启停操作共享同一受控目录。
    #[test]
    fn manages_global_skill_lifecycle() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let config = AppConfig::default();
        let initial = "---\nname: review\ndescription: Review code\n---\n\nInitial.";
        let id = create_managed_skill("review", initial, &paths).unwrap();
        assert_eq!(id, "global:review");
        assert!(list_managed_skills(&config, &paths).unwrap()[0].enabled);

        set_managed_skill_enabled(&id, false, &config, &paths).unwrap();
        assert!(!list_managed_skills(&config, &paths).unwrap()[0].enabled);

        let updated = "---\nname: review\ndescription: Review changes\n---\n\nUpdated.";
        update_managed_skill(&id, updated, &config, &paths).unwrap();
        assert_eq!(read_managed_skill(&id, &config, &paths).unwrap().1, updated);
    }
}
