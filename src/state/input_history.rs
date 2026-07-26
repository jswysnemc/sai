use crate::paths::SaiPaths;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// 跨会话共享的输入历史上限。
///
/// 上下键浏览超过这个量级已经不如直接重新输入，继续保留只会拖慢加载。
pub const INPUT_HISTORY_LIMIT: usize = 50;

/// 返回跨会话输入历史文件路径。
///
/// 放在全局 state 目录而不是会话目录下：新建会话后仍然能翻到之前输入过的内容。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - 历史文件路径
fn history_file(paths: &SaiPaths) -> PathBuf {
    paths.state_dir.join("input-history.jsonl")
}

/// 读取输入历史，按时间正序返回（最后一条为最近输入）。
///
/// 文件损坏或某行无法解析时跳过该行而不是整体失败：
/// 历史是辅助功能，不该因为一行坏数据挡住 REPL 启动。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - 历史输入列表，文件不存在时为空
pub fn load_input_history(paths: &SaiPaths) -> Result<Vec<String>> {
    let path = history_file(paths);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(&path)
        .with_context(|| format!("failed to read input history: {}", path.display()))?;
    Ok(parse_history(&content))
}

/// 追加一条输入历史。
///
/// 与最近一条完全相同时不重复记录，连续执行同一条命令不会挤占历史容量。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `entry`: 用户输入原文
///
/// 返回:
/// - 写入结果；空白输入直接跳过
pub fn append_input_history(paths: &SaiPaths, entry: &str) -> Result<()> {
    let trimmed = entry.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    let path = history_file(paths);
    let mut entries = load_input_history(paths)?;
    // 1. 与最近一条重复时不再追加
    if entries.last().map(String::as_str) == Some(trimmed) {
        return Ok(());
    }
    // 2. 同样的输入若出现在更早位置，移到末尾而不是留下两份
    entries.retain(|existing| existing != trimmed);
    entries.push(trimmed.to_string());
    // 3. 超出上限时丢弃最旧条目
    if entries.len() > INPUT_HISTORY_LIMIT {
        let excess = entries.len() - INPUT_HISTORY_LIMIT;
        entries.drain(..excess);
    }
    write_history(&path, &entries)
}

/// 解析历史文件内容。
///
/// 参数:
/// - `content`: 文件全文
///
/// 返回:
/// - 按文件顺序排列的历史输入
fn parse_history(content: &str) -> Vec<String> {
    content
        .lines()
        .filter_map(|line| serde_json::from_str::<String>(line).ok())
        .filter(|entry| !entry.trim().is_empty())
        .collect()
}

/// 覆盖写入历史文件。
///
/// 先写临时文件再改名：中途崩溃不会留下截断的历史。
///
/// 参数:
/// - `path`: 历史文件路径
/// - `entries`: 待写入的历史输入
///
/// 返回:
/// - 写入结果
fn write_history(path: &Path, entries: &[String]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create state dir: {}", parent.display()))?;
    }
    let mut body = String::new();
    for entry in entries {
        body.push_str(&serde_json::to_string(entry)?);
        body.push('\n');
    }
    let temp = path.with_extension("jsonl.tmp");
    fs::write(&temp, body)
        .with_context(|| format!("failed to write input history: {}", temp.display()))?;
    fs::rename(&temp, path)
        .with_context(|| format!("failed to replace input history: {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造指向临时目录的路径配置。
    ///
    /// 参数:
    /// - `root`: 临时根目录
    ///
    /// 返回:
    /// - 测试用路径配置
    fn test_paths(root: PathBuf) -> SaiPaths {
        SaiPaths {
            config_dir: root.join("config"),
            config_file: root.join("config/config.jsonc"),
            secrets_file: root.join("config/secrets.json"),
            skills_dir: root.join("skills"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            state_dir: root.join("state"),
            pictures_dir: root.join("pictures"),
            fish_hook_file: root.join("shell/sai.fish"),
            bash_hook_file: root.join("shell/bash-hook.sh"),
            zsh_hook_file: root.join("shell/zsh-hook.zsh"),
            powershell_hook_file: root.join("shell/powershell-hook.ps1"),
        }
    }

    /// 验证历史按时间正序累积且跨"会话"共享（同一路径重复读取）。
    #[test]
    fn appends_entries_in_order() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        append_input_history(&paths, "first").unwrap();
        append_input_history(&paths, "second").unwrap();
        assert_eq!(load_input_history(&paths).unwrap(), vec!["first", "second"]);
    }

    /// 验证连续重复输入不产生新条目。
    #[test]
    fn skips_consecutive_duplicates() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        append_input_history(&paths, "same").unwrap();
        append_input_history(&paths, "same").unwrap();
        assert_eq!(load_input_history(&paths).unwrap(), vec!["same"]);
    }

    /// 验证重复出现的旧输入被移动到末尾而不是留下两份。
    #[test]
    fn moves_repeated_entry_to_end() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        append_input_history(&paths, "a").unwrap();
        append_input_history(&paths, "b").unwrap();
        append_input_history(&paths, "a").unwrap();
        assert_eq!(load_input_history(&paths).unwrap(), vec!["b", "a"]);
    }

    /// 验证超出上限时丢弃最旧条目。
    #[test]
    fn trims_to_limit() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        for index in 0..(INPUT_HISTORY_LIMIT + 10) {
            append_input_history(&paths, &format!("entry-{index}")).unwrap();
        }
        let entries = load_input_history(&paths).unwrap();
        assert_eq!(entries.len(), INPUT_HISTORY_LIMIT);
        assert_eq!(entries.first().unwrap(), "entry-10");
        assert_eq!(
            entries.last().unwrap(),
            &format!("entry-{}", INPUT_HISTORY_LIMIT + 9)
        );
    }

    /// 验证空白输入不进入历史。
    #[test]
    fn ignores_blank_entries() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        append_input_history(&paths, "   \n  ").unwrap();
        assert!(load_input_history(&paths).unwrap().is_empty());
    }

    /// 验证损坏行被跳过而不是导致读取失败。
    #[test]
    fn skips_corrupt_lines() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        append_input_history(&paths, "good").unwrap();
        let path = history_file(&paths);
        let mut content = fs::read_to_string(&path).unwrap();
        content.push_str("{not json}\n");
        fs::write(&path, content).unwrap();
        assert_eq!(load_input_history(&paths).unwrap(), vec!["good"]);
    }
}
