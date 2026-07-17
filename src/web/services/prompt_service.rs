use crate::config::AppConfig;
use crate::paths::SaiPaths;
use anyhow::{bail, Result};
use serde::Serialize;
use std::path::PathBuf;

/// Web 提示词文件类型。
#[derive(Clone, Copy)]
pub(crate) enum PromptKind {
    Persona,
    Identity,
}

/// 提示词文件摘要。
#[derive(Clone, Debug, Serialize)]
pub(crate) struct PromptSummary {
    pub name: String,
}

/// 提示词文件内容。
#[derive(Clone, Debug, Serialize)]
pub(crate) struct PromptDocument {
    pub name: String,
    pub content: String,
}

/// 解析浏览器提交的提示词类型。
///
/// 参数:
/// - `value`: 路由中的类型文本
///
/// 返回:
/// - 对应提示词类型
pub(crate) fn parse_kind(value: &str) -> Result<PromptKind> {
    match value {
        "personas" => Ok(PromptKind::Persona),
        "identities" => Ok(PromptKind::Identity),
        _ => bail!("unsupported prompt kind: {value}"),
    }
}

/// 列出指定类型的提示词文件。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `kind`: 提示词类型
///
/// 返回:
/// - 按名称排序的文件摘要
pub(crate) fn list(paths: &SaiPaths, kind: PromptKind) -> Result<Vec<PromptSummary>> {
    let config = AppConfig::load_or_default(paths)?;
    let directory = prompt_directory(paths, &config, kind);
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let mut items = std::fs::read_dir(directory)?
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_type()
                .map(|kind| kind.is_file())
                .unwrap_or(false)
        })
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            name.ends_with(".md").then_some(PromptSummary {
                name: display_name(&name).to_string(),
            })
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(items)
}

/// 读取指定提示词文件。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `kind`: 提示词类型
/// - `name`: 提示词显示名称
///
/// 返回:
/// - 提示词文件内容
pub(crate) fn read(paths: &SaiPaths, kind: PromptKind, name: &str) -> Result<PromptDocument> {
    let config = AppConfig::load_or_default(paths)?;
    let file_name = sanitize_name(name)?;
    let path = prompt_path(paths, &config, kind, &file_name);
    if !path.is_file() {
        bail!("prompt not found: {}", display_name(&file_name));
    }
    Ok(PromptDocument {
        name: display_name(&file_name).to_string(),
        content: std::fs::read_to_string(path)?,
    })
}

/// 创建或更新提示词文件，并支持重命名。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `kind`: 提示词类型
/// - `current_name`: 当前名称，新建时为空
/// - `name`: 保存后的名称
/// - `content`: Markdown 内容
///
/// 返回:
/// - 保存后的提示词文件
pub(crate) fn save(
    paths: &SaiPaths,
    kind: PromptKind,
    current_name: Option<&str>,
    name: &str,
    content: &str,
) -> Result<PromptDocument> {
    let config = AppConfig::load_or_default(paths)?;
    let file_name = sanitize_name(name)?;
    let path = prompt_path(paths, &config, kind, &file_name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, format_content(content))?;
    if let Some(current_name) = current_name {
        let current_file_name = sanitize_name(current_name)?;
        if current_file_name != file_name {
            let current_path = prompt_path(paths, &config, kind, &current_file_name);
            if current_path.exists() {
                std::fs::remove_file(current_path)?;
            }
            if matches!(kind, PromptKind::Persona) {
                move_directory(
                    config.persona_memory_data_dir(paths, &current_file_name),
                    config.persona_memory_data_dir(paths, &file_name),
                )?;
                move_directory(
                    config.persona_memory_state_dir(paths, &current_file_name),
                    config.persona_memory_state_dir(paths, &file_name),
                )?;
                move_directory(
                    config.persona_skills_dir(paths, &current_file_name),
                    config.persona_skills_dir(paths, &file_name),
                )?;
            }
        }
    }
    read(paths, kind, &file_name)
}

/// 删除提示词文件。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `kind`: 提示词类型
/// - `name`: 提示词名称
///
/// 返回:
/// - 文件存在并删除时返回 true
pub(crate) fn remove(paths: &SaiPaths, kind: PromptKind, name: &str) -> Result<bool> {
    let config = AppConfig::load_or_default(paths)?;
    let file_name = sanitize_name(name)?;
    let path = prompt_path(paths, &config, kind, &file_name);
    if !path.exists() {
        return Ok(false);
    }
    std::fs::remove_file(path)?;
    if matches!(kind, PromptKind::Persona) {
        remove_directory(config.persona_memory_data_dir(paths, &file_name))?;
        remove_directory(config.persona_memory_state_dir(paths, &file_name))?;
        remove_directory(config.persona_skills_dir(paths, &file_name))?;
    }
    Ok(true)
}

/// 返回指定类型的提示词目录。
fn prompt_directory(paths: &SaiPaths, config: &AppConfig, kind: PromptKind) -> PathBuf {
    match kind {
        PromptKind::Persona => config.prompts_dir_path(paths),
        PromptKind::Identity => config.identities_dir_path(paths),
    }
}

/// 返回指定提示词文件路径。
fn prompt_path(paths: &SaiPaths, config: &AppConfig, kind: PromptKind, name: &str) -> PathBuf {
    prompt_directory(paths, config, kind).join(name)
}

/// 校验提示词名称并补充 Markdown 扩展名。
fn sanitize_name(value: &str) -> Result<String> {
    let value = value.trim().trim_end_matches(".md");
    if value.is_empty() {
        bail!("prompt name cannot be empty");
    }
    if value.contains(['/', '\\']) || value == "." || value == ".." {
        bail!("prompt name contains invalid path characters");
    }
    Ok(format!("{value}.md"))
}

/// 返回不含扩展名的提示词显示名称。
fn display_name(value: &str) -> &str {
    value.strip_suffix(".md").unwrap_or(value)
}

/// 统一提示词文本末尾换行。
fn format_content(content: &str) -> String {
    let content = content.trim_end();
    if content.is_empty() {
        String::new()
    } else {
        format!("{content}\n")
    }
}

/// 删除存在的提示词作用域目录。
fn remove_directory(path: PathBuf) -> Result<()> {
    if path.exists() {
        std::fs::remove_dir_all(path)?;
    }
    Ok(())
}

/// 移动存在的提示词作用域目录。
fn move_directory(from: PathBuf, to: PathBuf) -> Result<()> {
    if !from.exists() || from == to {
        return Ok(());
    }
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if to.exists() {
        std::fs::remove_dir_all(&to)?;
    }
    std::fs::rename(from, to)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_prompt_names() {
        assert_eq!(sanitize_name("coding").unwrap(), "coding.md");
        assert!(sanitize_name("../coding").is_err());
        assert!(sanitize_name("").is_err());
    }

    #[test]
    fn formats_prompt_content() {
        assert_eq!(format_content("hello\n\n"), "hello\n");
        assert_eq!(format_content(""), "");
    }
}
