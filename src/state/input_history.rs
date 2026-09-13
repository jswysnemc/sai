use crate::paths::SaiPaths;
use anyhow::{Context, Result};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

mod entry;
pub use entry::{InputHistoryAttachment, InputHistoryAttachmentKind, InputHistoryEntry};

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
    Ok(load_input_history_entries(paths)?
        .into_iter()
        .map(|entry| entry.text)
        .collect())
}

/// 【输入历史】【读取快照】读取正文及附件，兼容旧版每行只有字符串的历史文件。
/// 参数: `paths` 为应用路径
/// 返回: 按时间正序排列的完整输入快照
pub fn load_input_history_entries(paths: &SaiPaths) -> Result<Vec<InputHistoryEntry>> {
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
    append_input_history_entry(paths, &InputHistoryEntry::from(entry.to_string()))
}

/// 【输入历史】【保存快照】保存完整输入，去重时同时比较附件，避免同名标签覆盖不同原文。
/// 参数: `paths` 为应用路径，`entry` 为待保存快照
/// 返回: 写入结果
pub fn append_input_history_entry(paths: &SaiPaths, entry: &InputHistoryEntry) -> Result<()> {
    if entry.text.trim().is_empty() {
        return Ok(());
    }
    let path = history_file(paths);
    // 1. 【输入历史】【并发保存】串行更新共享索引，避免多个终端覆盖彼此的新条目
    fs::create_dir_all(&paths.state_dir)?;
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(paths.state_dir.join("input-history.lock"))?;
    lock.lock()?;
    let mut entries = load_input_history_entries(paths)?;
    // 2. 【输入历史】【保存快照】与最近一条重复时不再追加
    if entries.last() == Some(entry) {
        return Ok(());
    }
    // 3. 【输入历史】【保存快照】把较早的重复条目移到末尾
    push_input_history_entry(&mut entries, entry.clone());
    write_history(&path, &entries)
}

/// 【输入历史】【维护列表】按完整快照去重并限制容量，编辑历史不会修改先前的附件。
/// 参数: `entries` 为内存历史列表，`entry` 为独立快照
/// 返回: 无，原地更新列表
pub fn push_input_history_entry(entries: &mut Vec<InputHistoryEntry>, entry: InputHistoryEntry) {
    if entry.text.trim().is_empty() {
        return;
    }
    entries.retain(|existing| existing != &entry);
    entries.push(entry);
    // 1. 【输入历史】【维护列表】超出上限时丢弃最旧条目
    if entries.len() > INPUT_HISTORY_LIMIT {
        let excess = entries.len() - INPUT_HISTORY_LIMIT;
        entries.drain(..excess);
    }
}

/// 解析历史文件内容。
///
/// 参数:
/// - `content`: 文件全文
///
/// 返回:
/// - 按文件顺序排列的历史输入
fn parse_history(content: &str) -> Vec<InputHistoryEntry> {
    let mut entries: Vec<_> = content
        .lines()
        .filter_map(|line| {
            serde_json::from_str::<InputHistoryEntry>(line)
                .ok()
                .or_else(|| {
                    serde_json::from_str::<String>(line)
                        .ok()
                        .map(InputHistoryEntry::from)
                })
        })
        .filter(|entry| !entry.text.trim().is_empty())
        .collect();
    let excess = entries.len().saturating_sub(INPUT_HISTORY_LIMIT);
    entries.drain(..excess);
    entries
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
fn write_history(path: &Path, entries: &[InputHistoryEntry]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create state dir: {}", parent.display()))?;
    }
    let mut body = String::new();
    for entry in entries {
        if entry.attachments.is_empty() {
            body.push_str(&serde_json::to_string(&entry.text)?);
        } else {
            body.push_str(&serde_json::to_string(entry)?);
        }
        body.push('\n');
    }
    // 1. 【输入历史】【原子写入】使用独立临时文件，避免多个终端共享同一临时文件名
    let mut temp = tempfile::NamedTempFile::new_in(path.parent().unwrap_or(Path::new(".")))?;
    temp.write_all(body.as_bytes())?;
    temp.persist(path)
        .with_context(|| format!("failed to replace input history: {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests;
