use crate::paths::SaiPaths;
use crate::runtime_cwd;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// 全局附加指令文件名（位于 Sai 配置目录）。
const GLOBAL_INSTRUCTION_NAMES: &[&str] = &["AGENT.md", "AGENTS.md"];

/// 项目目录内按优先级查找的附加指令文件。
const PROJECT_INSTRUCTION_NAMES: &[&str] = &[
    ".AGENT.md",
    "AGENT.md",
    ".AGENTS.md",
    "AGENTS.md",
    ".CLAUDE.md",
    "CLAUDE.md",
];

/// 单文件最大读取字符数，避免把巨型文档塞进上下文。
const MAX_INSTRUCTION_CHARS: usize = 120_000;

/// 读取全局与项目附加系统提示词并组装为 XML 片段。
///
/// 加载顺序：
/// 1. `~/.config/sai/AGENT.md`（或 `AGENTS.md`）
/// 2. 从当前工作区根到 cwd 路径上的项目指令文件（根目录在前、近 cwd 在后）
///
/// 参数:
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 已拼接的附加提示文本；没有任何文件时返回空字符串
pub(super) fn load_instruction_prompt(paths: &SaiPaths) -> String {
    let mut sections = Vec::new();
    let mut seen_paths = HashSet::new();
    let mut seen_content = HashSet::new();

    // 1. 全局配置目录中的 AGENT.md
    for name in GLOBAL_INSTRUCTION_NAMES {
        let path = paths.config_dir.join(name);
        if let Some(section) = read_instruction_section(&path, "global", &mut seen_paths, &mut seen_content)
        {
            sections.push(section);
            break;
        }
    }

    // 2. 当前工作目录向上收集项目指令，再反转为根 → 叶子
    if let Ok(cwd) = runtime_cwd::current_dir() {
        let mut project_sections = Vec::new();
        for dir in walk_dirs_upward(&cwd) {
            if let Some(path) = find_project_instruction(&dir) {
                if let Some(section) =
                    read_instruction_section(&path, "project", &mut seen_paths, &mut seen_content)
                {
                    project_sections.push(section);
                }
            }
        }
        project_sections.reverse();
        sections.extend(project_sections);
    }

    if sections.is_empty() {
        return String::new();
    }

    let mut out = String::from("<instruction-files>\n");
    out.push_str("Additional instructions from global and project instruction files.\n");
    out.push_str("Prefer closer project files when they conflict with higher-level ones.\n\n");
    out.push_str(&sections.join("\n\n"));
    out.push_str("\n</instruction-files>");
    out
}

/// 在目录中按优先级查找项目指令文件。
fn find_project_instruction(dir: &Path) -> Option<PathBuf> {
    for name in PROJECT_INSTRUCTION_NAMES {
        let path = dir.join(name);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

/// 从 cwd 向上遍历到根目录（含 cwd）。
fn walk_dirs_upward(start: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut current = start.to_path_buf();
    loop {
        dirs.push(current.clone());
        if !current.pop() {
            break;
        }
        // Unix 根 `/` 时 pop 后仍可能是自己；PathBuf::pop 对根返回 false
    }
    dirs
}

/// 读取单个指令文件并包装为带路径标记的片段。
fn read_instruction_section(
    path: &Path,
    scope: &str,
    seen_paths: &mut HashSet<PathBuf>,
    seen_content: &mut HashSet<u64>,
) -> Option<String> {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if !seen_paths.insert(canonical.clone()) {
        return None;
    }
    let raw = std::fs::read_to_string(path).ok()?;
    let content = trim_instruction(&raw);
    if content.is_empty() {
        return None;
    }
    let hash = content_hash(content);
    if !seen_content.insert(hash) {
        return None;
    }
    let display = path.display();
    Some(format!(
        "<instruction-file scope=\"{scope}\" path=\"{display}\">\n{content}\n</instruction-file>"
    ))
}

/// 裁剪空白并限制长度。
fn trim_instruction(raw: &str) -> &str {
    let trimmed = raw.trim();
    if trimmed.chars().count() <= MAX_INSTRUCTION_CHARS {
        return trimmed;
    }
    // 按字符截断，避免切开 UTF-8
    let end = trimmed
        .char_indices()
        .nth(MAX_INSTRUCTION_CHARS)
        .map(|(index, _)| index)
        .unwrap_or(trimmed.len());
    trimmed[..end].trim_end()
}

/// 内容指纹，用于去重。
fn content_hash(content: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::{Mutex, OnceLock};

    fn cwd_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn test_paths(root: PathBuf) -> SaiPaths {
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
    fn loads_global_and_project_instruction_files() {
        let _guard = cwd_lock().lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let config_dir = temp.path().join("config");
        let project = temp.path().join("project");
        let nested = project.join("src");
        fs::create_dir_all(&config_dir).unwrap();
        fs::create_dir_all(&nested).unwrap();
        fs::write(config_dir.join("AGENT.md"), "global rules").unwrap();
        fs::write(project.join(".AGENT.md"), "project rules").unwrap();
        fs::write(nested.join(".CLAUDE.md"), "nested rules").unwrap();

        let paths = test_paths(temp.path().to_path_buf());
        let previous = std::env::current_dir().unwrap();
        std::env::set_current_dir(&nested).unwrap();
        let prompt = load_instruction_prompt(&paths);
        let _ = std::env::set_current_dir(previous);

        assert!(prompt.contains("global rules"));
        assert!(prompt.contains("project rules"));
        assert!(prompt.contains("nested rules"));
        assert!(prompt.contains("<instruction-files>"));
        // 根项目文件应出现在嵌套文件之前
        let project_pos = prompt.find("project rules").unwrap();
        let nested_pos = prompt.find("nested rules").unwrap();
        assert!(project_pos < nested_pos);
    }

    #[test]
    fn prefers_first_matching_project_name_in_directory() {
        let _guard = cwd_lock().lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("repo");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join(".AGENT.md"), "agent file").unwrap();
        fs::write(project.join(".CLAUDE.md"), "claude file").unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        let previous = std::env::current_dir().unwrap();
        std::env::set_current_dir(&project).unwrap();
        let prompt = load_instruction_prompt(&paths);
        let _ = std::env::set_current_dir(previous);
        assert!(prompt.contains("agent file"));
        assert!(!prompt.contains("claude file"));
    }

    #[test]
    fn skips_missing_files_quietly() {
        let _guard = cwd_lock().lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("empty");
        fs::create_dir_all(&project).unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        let previous = std::env::current_dir().unwrap();
        std::env::set_current_dir(&project).unwrap();
        let prompt = load_instruction_prompt(&paths);
        let _ = std::env::set_current_dir(previous);
        assert!(prompt.is_empty());
    }
}
