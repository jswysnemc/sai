use super::{MentionSuggestion, MAX_REPL_COMMAND_SUGGESTIONS};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// 【终端】【文件补全】后台线程独占目录缓存，不在界面线程访问文件系统
pub(super) struct DirectoryCache {
    root: PathBuf,
    prefix: String,
    hidden: bool,
    fetched: Instant,
    entries: Vec<MentionSuggestion>,
}

/// 查找文件候选；cwd 为工作目录，query 为查询，cache 为目录缓存，cancelled 为取消检查
/// 返回已排序的可见候选；取消时返回空值且不缓存不完整结果
pub(super) fn complete(
    cwd: &Path,
    query: &str,
    cache: &mut Option<DirectoryCache>,
    cancelled: impl Fn() -> bool,
) -> Option<Vec<MentionSuggestion>> {
    let (prefix, filter) = match query.rfind('/') {
        Some(index) => (&query[..=index], &query[index + 1..]),
        None => ("", query),
    };
    let root = cwd.join(prefix);
    let hidden = filter.starts_with('.')
        || prefix
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .is_some_and(|part| part.starts_with('.'));
    let valid = cache.as_ref().is_some_and(|cached| {
        cached.root == root
            && cached.prefix == prefix
            && cached.hidden == hidden
            && cached.fetched.elapsed() < Duration::from_millis(500)
    });
    if !valid {
        let entries = read_entries(&root, prefix, hidden, &cancelled)?;
        *cache = Some(DirectoryCache {
            root,
            prefix: prefix.to_string(),
            hidden,
            fetched: Instant::now(),
            entries,
        });
    }
    let keyword = filter.to_ascii_lowercase();
    let mut result = Vec::new();
    for entry in &cache.as_ref()?.entries {
        if cancelled() {
            return None;
        }
        if keyword.is_empty() || entry.label.to_ascii_lowercase().contains(&keyword) {
            result.push(entry.clone());
            if result.len() == MAX_REPL_COMMAND_SUGGESTIONS {
                break;
            }
        }
    }
    Some(result)
}

/// 读取目录条目；root/prefix 为路径及插入前缀，hidden 控制隐藏文件，cancelled 检查取消
/// 返回目录优先的排序结果；遇到取消返回空值
fn read_entries(
    root: &Path,
    prefix: &str,
    hidden: bool,
    cancelled: &impl Fn() -> bool,
) -> Option<Vec<MentionSuggestion>> {
    if cancelled() {
        return None;
    }
    let Ok(directory) = std::fs::read_dir(root) else {
        return Some(Vec::new());
    };
    let mut entries = Vec::new();
    for entry in directory.flatten() {
        if cancelled() {
            return None;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if (!hidden && name.starts_with('.'))
            || matches!(
                name.as_str(),
                ".git" | "node_modules" | "target" | "dist" | "build" | ".sai" | "__pycache__"
            )
        {
            continue;
        }
        let is_dir = entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false);
        let relative = format!("{prefix}{name}{}", if is_dir { "/" } else { "" });
        entries.push(MentionSuggestion {
            insert: format!("@{relative}"),
            label: relative,
            description: if is_dir { "directory" } else { "file" }.into(),
            continue_filter: is_dir,
        });
    }
    entries.sort_by(|a, b| {
        b.continue_filter
            .cmp(&a.continue_filter)
            .then(a.label.cmp(&b.label))
    });
    (!cancelled()).then_some(entries)
}
