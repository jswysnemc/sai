use super::{ToolRegistry, ToolSpec};
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;

pub fn skills_prompt(config: &AppConfig, paths: &SaiPaths) -> Result<String> {
    let entries = visible_skill_entries(config, paths)?
        .into_iter()
        .map(|(entry, full)| {
            if !full {
                return format!("- {}", entry.name);
            }
            format!(
                "- {}: {}\n  {}",
                entry.name,
                entry.description,
                compact_skill_body(&entry.body)
            )
        })
        .collect::<Vec<_>>();
    if entries.is_empty() {
        return Ok(String::new());
    }
    Ok(format!(
        "<available-skills>\n这些是已安装的 skills。遇到匹配任务时主动参考。当前不支持创建、保存或自动生成新的 skill；不要把 skill 内容保存到知识库。\n{}\n</available-skills>",
        entries.join("\n")
    ))
}

pub fn skills_catalog_prompt(config: &AppConfig, paths: &SaiPaths) -> Result<String> {
    let entries = visible_skill_entries(config, paths)?
        .into_iter()
        .map(|(entry, full)| {
            if full {
                format!("- {} [{}]: {}", entry.name, entry.source, entry.description)
            } else {
                format!("- {}", entry.name)
            }
        })
        .collect::<Vec<_>>();
    if entries.is_empty() {
        return Ok(String::new());
    }
    Ok(format!(
        "<available-skills>\n这些是已安装的 skills 目录。默认只提供名称和简介；需要使用完整流程时，调用 load，设置 type 为 skill，并通过 keywords 数组传入名称。\n{}\n</available-skills>",
        entries.join("\n")
    ))
}

pub fn register_skills(
    registry: &mut ToolRegistry,
    config: &AppConfig,
    paths: &SaiPaths,
    allow_command_execution: bool,
) -> Result<()> {
    let mut seen = BTreeSet::new();
    for skills_dir in skill_search_dirs(config, paths) {
        if !skills_dir.exists() {
            continue;
        }
        for entry in std::fs::read_dir(&skills_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let skill_dir = entry.path();
            if skill_dir.join(".disabled").exists() {
                continue;
            }
            let skill_file = skill_dir.join("SKILL.md");
            if !skill_file.is_file() {
                continue;
            }
            let raw = std::fs::read_to_string(&skill_file)?;
            let name = skill_name(&raw, &entry.file_name().to_string_lossy());
            if !seen.insert(name.clone()) {
                continue;
            }
            if name == "web-search" {
                register_web_search(registry, skill_dir, allow_command_execution);
            }
        }
    }
    Ok(())
}

fn skill_search_dirs(config: &AppConfig, paths: &SaiPaths) -> Vec<PathBuf> {
    let mut dirs = vec![paths.skills_dir.clone()];
    let active = config.active_persona_skills_dir(paths);
    if active != paths.skills_dir {
        dirs.push(active);
    }
    dirs
}

struct SkillEntry {
    name: String,
    description: String,
    body: String,
    raw: String,
    source: &'static str,
    dir: PathBuf,
    file: PathBuf,
}

/// 读取所有启用的 skill 条目。
///
/// 参数:
/// - `config`: 当前应用配置
/// - `paths`: 应用目录路径集合
///
/// 返回:
/// - 按搜索目录优先级去重后的 skill 条目
fn skill_entries(config: &AppConfig, paths: &SaiPaths) -> Result<Vec<SkillEntry>> {
    let mut entries = Vec::new();
    let mut seen = BTreeSet::new();
    let search_dirs = skill_search_dirs(config, paths);
    for skills_dir in search_dirs {
        if !skills_dir.exists() {
            continue;
        }
        let source = if skills_dir == paths.skills_dir {
            "global"
        } else {
            "persona"
        };
        for entry in std::fs::read_dir(&skills_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() || entry.path().join(".disabled").exists() {
                continue;
            }
            let skill_file = entry.path().join("SKILL.md");
            if !skill_file.is_file() {
                continue;
            }
            let raw = std::fs::read_to_string(&skill_file)?;
            let name = skill_name(&raw, &entry.file_name().to_string_lossy());
            if !seen.insert(name.clone()) {
                continue;
            }
            let description = frontmatter_value(&raw, "description").unwrap_or_default();
            let body = strip_frontmatter(&raw);
            entries.push(SkillEntry {
                name,
                description,
                body,
                raw,
                source,
                dir: entry.path(),
                file: skill_file,
            });
        }
    }
    Ok(entries)
}

/// 按当前 Agent 策略筛选 skills，并标记是否完整暴露。
///
/// 参数:
/// - `config`: 当前运行配置
/// - `paths`: 应用目录路径集合
///
/// 返回:
/// - 可见 skill 及完整暴露标记；未选择 Agent 时全部完整暴露
fn visible_skill_entries(config: &AppConfig, paths: &SaiPaths) -> Result<Vec<(SkillEntry, bool)>> {
    let entries = skill_entries(config, paths)?;
    let Some(runtime) = config.agent_runtime.as_ref() else {
        return Ok(entries.into_iter().map(|entry| (entry, true)).collect());
    };
    Ok(entries
        .into_iter()
        .filter_map(|entry| {
            if runtime.skills_full.contains(&entry.name) {
                Some((entry, true))
            } else if runtime.skills_named.contains(&entry.name) {
                Some((entry, false))
            } else {
                None
            }
        })
        .collect())
}

/// Skill 目录条目，仅包含名称与描述。
pub struct SkillCatalogEntry {
    /// Skill 名称
    pub name: String,
    /// Skill 简介
    pub description: String,
}

/// 枚举当前可用的 skill 名称与描述。
///
/// 参数:
/// - `config`: 当前应用配置
/// - `paths`: 应用目录路径集合
///
/// 返回:
/// - 去重后的 skill 目录条目列表
pub fn skill_catalog(config: &AppConfig, paths: &SaiPaths) -> Result<Vec<SkillCatalogEntry>> {
    // 1. 复用 skill 发现逻辑读取全部条目
    let entries = skill_entries(config, paths)?;
    // 2. 只保留名称与描述返回
    Ok(entries
        .into_iter()
        .map(|entry| SkillCatalogEntry {
            name: entry.name,
            description: entry.description,
        })
        .collect())
}

fn skill_name(raw: &str, fallback: &str) -> String {
    frontmatter_value(raw, "name").unwrap_or_else(|| fallback.to_string())
}

/// 按名称读取完整 skill 文档。
///
/// 参数:
/// - `name`: skill 名称
/// - `config`: 当前应用配置
/// - `paths`: 应用目录路径集合
///
/// 返回:
/// - 完整 `SKILL.md` 文本
pub(crate) fn load_installed_skill(
    name: &str,
    config: &AppConfig,
    paths: &SaiPaths,
) -> Result<String> {
    load_skill_document(name, config, paths, true)
}

/// 按名称读取完整 skill 文档，供 Web 输入区显式引用。
///
/// 与 `load_installed_skill` 不同，这里不按当前 Agent 权限过滤，
/// 因为用户已在 UI 中主动选择该 skill。
///
/// 参数:
/// - `name`: skill 名称
/// - `config`: 当前应用配置
/// - `paths`: 应用目录路径集合
///
/// 返回:
/// - 完整 `SKILL.md` 文本
pub fn load_installed_skill_document(
    name: &str,
    config: &AppConfig,
    paths: &SaiPaths,
) -> Result<String> {
    load_skill_document(name, config, paths, false)
}

/// 按名称读取完整 skill 文档。
///
/// 参数:
/// - `name`: skill 名称
/// - `config`: 当前应用配置
/// - `paths`: 应用目录路径集合
/// - `respect_agent_visibility`: 是否只允许当前 Agent 可见 skill
///
/// 返回:
/// - 完整 `SKILL.md` 文本
fn load_skill_document(
    name: &str,
    config: &AppConfig,
    paths: &SaiPaths,
    respect_agent_visibility: bool,
) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        bail!("load requires a non-empty skill_name");
    }
    let entries = if respect_agent_visibility {
        visible_skill_entries(config, paths)?
            .into_iter()
            .map(|(entry, _)| entry)
            .collect::<Vec<_>>()
    } else {
        skill_entries(config, paths)?
    };
    for entry in entries {
        if entry.name == name {
            return Ok(format!(
                "<loaded-skill name=\"{}\" source=\"{}\" dir=\"{}\" file=\"{}\">\n<skill-location>\nSkill directory: {}\nSkill file: {}\nResolve relative paths in this skill against the skill directory.\n</skill-location>\n{}\n</loaded-skill>",
                entry.name,
                entry.source,
                entry.dir.display(),
                entry.file.display(),
                entry.dir.display(),
                entry.file.display(),
                entry.raw.trim()
            ));
        }
    }
    bail!("skill not found: {name}");
}

fn register_web_search(
    registry: &mut ToolRegistry,
    skill_dir: PathBuf,
    allow_command_execution: bool,
) {
    let script = skill_dir.join("scripts/web-search.py");
    registry.register(ToolSpec::new(
        "web_search",
        "Search the web for current or real-time information. Use this when the answer needs online lookup, recent facts, news, or verification. Return search results with URLs for verification when needed.",
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search query." },
                "max_results": { "type": "integer", "description": "Maximum results to return.", "minimum": 1, "maximum": 10 },
                "provider": { "type": "string", "enum": ["auto", "tavily", "firecrawl", "anysearch", "searxng"], "description": "Search provider." }
            },
            "required": ["query"],
            "additionalProperties": false
        }),
        move |args| {
            let script = script.clone();
            async move { run_web_search(script, allow_command_execution, args).await }
        },
    ));
}

async fn run_web_search(
    script: PathBuf,
    allow_command_execution: bool,
    args: Value,
) -> Result<String> {
    if !allow_command_execution {
        bail!("skill command execution is disabled; set skills.allow_command_execution=true in config.jsonc to enable this tool");
    }
    if !script.is_file() {
        bail!("web-search skill script not found: {}", script.display());
    }
    let query = args
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if query.is_empty() {
        bail!("web_search requires a non-empty query");
    }
    let max_results = args
        .get("max_results")
        .and_then(Value::as_u64)
        .unwrap_or(5)
        .clamp(1, 10)
        .to_string();
    let provider = args
        .get("provider")
        .and_then(Value::as_str)
        .unwrap_or("auto");
    let output = run_python_script(&script, &[query, "-n", &max_results, "-p", provider]).await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("web_search failed: {}", stderr.trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

async fn run_python_script(script: &PathBuf, args: &[&str]) -> Result<std::process::Output> {
    let mut missing = Vec::new();
    for launcher in python_launchers() {
        let mut command = Command::new(launcher.program);
        command.args(launcher.prefix_args).arg(script).args(args);
        match command.stdin(Stdio::null()).output().await {
            Ok(output) => return Ok(output),
            Err(err) if err.kind() == ErrorKind::NotFound => {
                missing.push(launcher.label());
            }
            Err(err) => return Err(err.into()),
        }
    }
    bail!("Python launcher not found; tried {}", missing.join(", "))
}

#[derive(Clone, Copy)]
struct PythonLauncher {
    program: &'static str,
    prefix_args: &'static [&'static str],
}

impl PythonLauncher {
    fn label(self) -> String {
        if self.prefix_args.is_empty() {
            self.program.to_string()
        } else {
            format!("{} {}", self.program, self.prefix_args.join(" "))
        }
    }
}

#[cfg(windows)]
fn python_launchers() -> Vec<PythonLauncher> {
    vec![
        PythonLauncher {
            program: "py",
            prefix_args: &["-3"],
        },
        PythonLauncher {
            program: "python",
            prefix_args: &[],
        },
        PythonLauncher {
            program: "python3",
            prefix_args: &[],
        },
    ]
}

#[cfg(not(windows))]
fn python_launchers() -> Vec<PythonLauncher> {
    vec![
        PythonLauncher {
            program: "python3",
            prefix_args: &[],
        },
        PythonLauncher {
            program: "python",
            prefix_args: &[],
        },
    ]
}

fn frontmatter_value(raw: &str, key: &str) -> Option<String> {
    let mut lines = raw.lines();
    if lines.next()? != "---" {
        return None;
    }
    for line in lines {
        if line == "---" {
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

fn strip_frontmatter(raw: &str) -> String {
    let mut lines = raw.lines();
    if lines.next() != Some("---") {
        return raw.to_string();
    }
    for line in lines.by_ref() {
        if line == "---" {
            return lines.collect::<Vec<_>>().join("\n");
        }
    }
    raw.to_string()
}

fn compact_skill_body(body: &str) -> String {
    let text = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() > 700 {
        format!("{}...", text.chars().take(697).collect::<String>())
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_paths(root: &std::path::Path) -> SaiPaths {
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

    #[test]
    fn skills_prompt_reads_global_skills_dir() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let skill_dir = paths.skills_dir.join("gpu-passthrough");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: gpu-passthrough\ndescription: GPU switching\n---\n\nUse `gpustoggle --status`.",
        )
        .unwrap();
        let config = AppConfig::default();
        let prompt = skills_prompt(&config, &paths).unwrap();
        assert!(prompt.contains("gpu-passthrough"));
        assert!(prompt.contains("GPU switching"));
    }

    #[test]
    fn skills_catalog_omits_full_body() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let skill_dir = paths.skills_dir.join("gpu-passthrough");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: gpu-passthrough\ndescription: GPU switching\n---\n\nUse `gpustoggle --status`.",
        )
        .unwrap();
        let config = AppConfig::default();
        let prompt = skills_catalog_prompt(&config, &paths).unwrap();

        assert!(prompt.contains("gpu-passthrough"));
        assert!(prompt.contains("GPU switching"));
        assert!(!prompt.contains("gpustoggle --status"));
    }

    #[test]
    fn load_installed_skill_returns_full_skill_file() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let skill_dir = paths.skills_dir.join("gpu-passthrough");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: gpu-passthrough\ndescription: GPU switching\n---\n\nUse `gpustoggle --status`.",
        )
        .unwrap();
        let config = AppConfig::default();
        let output = load_installed_skill("gpu-passthrough", &config, &paths).unwrap();

        assert!(output.contains("<loaded-skill"));
        assert!(output.contains("Skill directory:"));
        assert!(output.contains("Skill file:"));
        assert!(
            output.contains("Resolve relative paths in this skill against the skill directory.")
        );
        assert!(output.contains(&skill_dir.display().to_string()));
        assert!(output.contains("gpustoggle --status"));
    }

    #[test]
    fn load_installed_skill_rejects_hidden_agent_skill() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let skill_dir = paths.skills_dir.join("gpu-passthrough");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: gpu-passthrough\ndescription: GPU switching\n---\n\nUse `gpustoggle --status`.",
        )
        .unwrap();
        let mut config = AppConfig::default();
        config.agent_runtime = Some(crate::config::AgentRuntimeOverride {
            enabled_tools: Vec::new(),
            skills_full: Vec::new(),
            skills_named: Vec::new(),
        });

        let error = load_installed_skill("gpu-passthrough", &config, &paths).unwrap_err();

        assert!(error.to_string().contains("skill not found"));
    }

    #[test]
    fn python_launchers_match_platform_conventions() {
        let labels = python_launchers()
            .into_iter()
            .map(PythonLauncher::label)
            .collect::<Vec<_>>();
        #[cfg(windows)]
        assert_eq!(labels, vec!["py -3", "python", "python3"]);
        #[cfg(not(windows))]
        assert_eq!(labels, vec!["python3", "python"]);
    }
}
