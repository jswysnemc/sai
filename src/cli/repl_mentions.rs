use crate::cli::repl_commands::MAX_REPL_COMMAND_SUGGESTIONS;
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::runtime_cwd;
use crate::tools::{load_installed_skill_document, skill_catalog};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// 目录条目缓存有效期。
const DIR_CACHE_TTL: Duration = Duration::from_millis(500);

/// 单条目录列表缓存。
struct DirCacheEntry {
    /// 缓存键：目录路径 + 插入前缀
    key: String,
    show_hidden: bool,
    fetched_at: Instant,
    entries: Vec<MentionSuggestion>,
}

/// 目录条目缓存。
///
/// 补全面板每帧都会重算（32ms 一次），而 read_dir + 逐条 stat 在大目录上
/// 可能要几十毫秒，未缓存时输入 `@` 会直接把界面卡住。
static DIR_CACHE: Mutex<Option<DirCacheEntry>> = Mutex::new(None);

/// 输入框引用触发类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MentionKind {
    /// `#` 引入 skill
    Skill,
    /// `@` 引入当前目录文件
    File,
}

/// 光标处的引用触发范围。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MentionTrigger {
    pub kind: MentionKind,
    pub start: usize,
    pub end: usize,
    pub query: String,
}

/// 一条可插入的引用建议。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MentionSuggestion {
    pub insert: String,
    pub label: String,
    pub description: String,
    pub continue_filter: bool,
}

/// 查找光标前的 `#` / `@` 触发片段。
///
/// `#` / `@` 必须位于行首或空白之后，避免邮件地址或标签中间误触发。
///
/// 参数:
/// - `input`: 当前输入
/// - `cursor`: 光标字符偏移
///
/// 返回:
/// - 触发范围与过滤词
pub(super) fn find_mention_trigger(input: &str, cursor: usize) -> Option<MentionTrigger> {
    let chars: Vec<char> = input.chars().collect();
    if cursor == 0 || cursor > chars.len() {
        return None;
    }
    let mut start = cursor;
    while start > 0 && !is_trigger_boundary(chars[start - 1]) {
        start -= 1;
    }
    if start >= cursor {
        return None;
    }
    let kind = match chars[start] {
        '#' => MentionKind::Skill,
        '@' => MentionKind::File,
        _ => return None,
    };
    Some(MentionTrigger {
        kind,
        start,
        end: cursor,
        query: chars[start + 1..cursor].iter().collect(),
    })
}

/// 根据触发词生成可见建议。
///
/// 参数:
/// - `trigger`: 当前触发范围
/// - `skills`: 已缓存的 skill 名称与描述
///
/// 返回:
/// - 不超过面板容量的建议
pub(super) fn mention_suggestions(
    trigger: &MentionTrigger,
    skills: &[(String, String)],
) -> Vec<MentionSuggestion> {
    match trigger.kind {
        MentionKind::Skill => filter_skills(skills, &trigger.query),
        MentionKind::File => list_cwd_files(&trigger.query),
    }
}

/// 用选中项替换触发片段。
///
/// 参数:
/// - `input`: 当前输入
/// - `trigger`: 被替换的触发范围
/// - `item`: 选中的建议
///
/// 返回:
/// - 新输入与新光标位置
pub(super) fn apply_mention(
    input: &str,
    trigger: &MentionTrigger,
    item: &MentionSuggestion,
) -> (String, usize) {
    let chars: Vec<char> = input.chars().collect();
    let mut next = String::new();
    next.extend(chars.iter().take(trigger.start));
    next.push_str(&item.insert);
    if !item.continue_filter && !item.insert.ends_with(char::is_whitespace) {
        next.push(' ');
    }
    let cursor = next.chars().count();
    next.extend(chars.iter().skip(trigger.end));
    (next, cursor)
}

/// 提交前把 `#skill` 展开为完整 skill 文档。
///
/// 参数:
/// - `input`: 用户提交文本
/// - `config`: 当前配置
/// - `paths`: Sai 路径
///
/// 返回:
/// - 展开后的模型输入
pub(super) fn expand_skill_mentions(input: &str, config: &AppConfig, paths: &SaiPaths) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut output = String::new();
    let mut index = 0usize;
    while index < chars.len() {
        if chars[index] == '#'
            && (index == 0 || is_trigger_boundary(chars[index - 1]))
            && index + 1 < chars.len()
            && is_skill_name_char(chars[index + 1])
        {
            let mut end = index + 1;
            while end < chars.len() && is_skill_name_char(chars[end]) {
                end += 1;
            }
            let name: String = chars[index + 1..end].iter().collect();
            if let Ok(document) = load_installed_skill_document(&name, config, paths) {
                if !document.trim().is_empty() {
                    output.push_str(&format!(
                        "<skill-reference name=\"{name}\">\n{}\n</skill-reference>",
                        document.trim()
                    ));
                    index = end;
                    continue;
                }
            }
        }
        output.push(chars[index]);
        index += 1;
    }
    output
}

/// 判断字符是否可作为引用触发边界。
///
/// 参数:
/// - `ch`: 待判断字符
///
/// 返回:
/// - 空白视为边界
fn is_trigger_boundary(ch: char) -> bool {
    ch.is_whitespace()
}

/// 判断字符是否属于 skill 名称。
///
/// 参数:
/// - `ch`: 待判断字符
///
/// 返回:
/// - 是否为合法 skill 名称字符
fn is_skill_name_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-'
}

/// 按名称或描述过滤 skill。
///
/// 参数:
/// - `skills`: skill 目录
/// - `query`: 过滤词
///
/// 返回:
/// - 匹配的 skill 建议
fn filter_skills(skills: &[(String, String)], query: &str) -> Vec<MentionSuggestion> {
    let keyword = query.trim().to_ascii_lowercase();
    skills
        .iter()
        .filter(|(name, description)| {
            keyword.is_empty()
                || name.to_ascii_lowercase().contains(&keyword)
                || description.to_ascii_lowercase().contains(&keyword)
        })
        .take(MAX_REPL_COMMAND_SUGGESTIONS)
        .map(|(name, description)| MentionSuggestion {
            insert: format!("#{name}"),
            label: format!("#{name}"),
            description: description.clone(),
            continue_filter: false,
        })
        .collect()
}

/// 列出当前目录（或查询前缀目录）中匹配的文件与子目录。
///
/// 参数:
/// - `query`: `@` 后的过滤词，可含路径前缀
///
/// 返回:
/// - 匹配的文件建议
fn list_cwd_files(query: &str) -> Vec<MentionSuggestion> {
    let cwd = runtime_cwd::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let (dir, filter) = split_file_query(query);
    let root = if dir.is_empty() {
        cwd.clone()
    } else {
        cwd.join(&dir)
    };
    let keyword = filter.to_ascii_lowercase();
    let show_hidden = filter.starts_with('.')
        || dir
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .is_some_and(|part| part.starts_with('.'));
    let mut entries = cached_dir_entries(&root, &dir, show_hidden);
    if !keyword.is_empty() {
        entries.retain(|entry| entry.label.to_ascii_lowercase().contains(&keyword));
    }
    entries.truncate(MAX_REPL_COMMAND_SUGGESTIONS);
    entries
}

/// 把查询拆成目录前缀和最后一段过滤词。
///
/// 参数:
/// - `query`: `@` 后的文本
///
/// 返回:
/// - 相对目录与文件名过滤词
fn split_file_query(query: &str) -> (String, String) {
    match query.rfind('/') {
        Some(index) => (query[..=index].to_string(), query[index + 1..].to_string()),
        None => (String::new(), query.to_string()),
    }
}

/// 读取目录条目并在短时间内复用结果。
///
/// 过滤词每次按键都变，但目录内容不会，因此按目录缓存原始条目、
/// 过滤留在缓存之外做，避免每帧重复扫盘。
///
/// 参数:
/// - `root`: 绝对或工作区路径
/// - `prefix`: 插入时使用的相对前缀
/// - `show_hidden`: 是否列出点开头的条目
///
/// 返回:
/// - 已排序的目录与文件建议
fn cached_dir_entries(root: &Path, prefix: &str, show_hidden: bool) -> Vec<MentionSuggestion> {
    let key = format!("{}\u{0}{prefix}", root.display());
    if let Ok(guard) = DIR_CACHE.lock() {
        if let Some(entry) = guard.as_ref() {
            if entry.key == key
                && entry.show_hidden == show_hidden
                && entry.fetched_at.elapsed() < DIR_CACHE_TTL
            {
                return entry.entries.clone();
            }
        }
    }
    let entries = read_dir_entries(root, prefix, show_hidden);
    if let Ok(mut guard) = DIR_CACHE.lock() {
        *guard = Some(DirCacheEntry {
            key,
            show_hidden,
            fetched_at: Instant::now(),
            entries: entries.clone(),
        });
    }
    entries
}

/// 读取一个目录下的文件与子目录。
///
/// 参数:
/// - `root`: 绝对或工作区路径
/// - `prefix`: 插入时使用的相对前缀，含尾部 `/`
/// - `show_hidden`: 是否列出点开头的条目
///
/// 返回:
/// - 已排序的目录与文件建议
fn read_dir_entries(root: &Path, prefix: &str, show_hidden: bool) -> Vec<MentionSuggestion> {
    let Ok(read) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    for entry in read.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == "." || name == ".." {
            continue;
        }
        if !show_hidden && name.starts_with('.') {
            continue;
        }
        if is_ignored_dir(&name) {
            continue;
        }
        let is_dir = entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false);
        let relative = format!("{prefix}{name}");
        if is_dir {
            dirs.push(MentionSuggestion {
                insert: format!("@{relative}/"),
                label: format!("{relative}/"),
                description: "directory".to_string(),
                continue_filter: true,
            });
        } else {
            files.push(MentionSuggestion {
                insert: format!("@{relative}"),
                label: relative,
                description: "file".to_string(),
                continue_filter: false,
            });
        }
    }
    dirs.sort_by(|left, right| left.label.cmp(&right.label));
    files.sort_by(|left, right| left.label.cmp(&right.label));
    dirs.extend(files);
    dirs
}

/// 判断目录名是否应跳过。
///
/// 参数:
/// - `name`: 目录名
///
/// 返回:
/// - 常见构建与依赖目录为真
fn is_ignored_dir(name: &str) -> bool {
    matches!(
        name,
        ".git" | "node_modules" | "target" | "dist" | "build" | ".sai" | "__pycache__"
    )
}

/// 读取可供 TUI 引用的 skill 目录。
///
/// 参数:
/// - `config`: 当前配置
/// - `paths`: Sai 路径
///
/// 返回:
/// - 名称与描述列表
pub(super) fn load_mention_skills(config: &AppConfig, paths: &SaiPaths) -> Vec<(String, String)> {
    skill_catalog(config, paths)
        .unwrap_or_default()
        .into_iter()
        .map(|entry| (entry.name, entry.description))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证行首和空白后的 `#` / `@` 能触发，词中不会误触发。
    #[test]
    fn detects_hash_and_at_after_boundary() {
        assert_eq!(
            find_mention_trigger("#", 1).map(|item| (item.kind, item.query)),
            Some((MentionKind::Skill, String::new()))
        );
        assert_eq!(
            find_mention_trigger("看 #dra", 6).map(|item| item.query),
            Some("dra".to_string())
        );
        assert_eq!(
            find_mention_trigger("@src/", 5).map(|item| (item.kind, item.query)),
            Some((MentionKind::File, "src/".to_string()))
        );
        assert!(find_mention_trigger("user@host", 9).is_none());
        assert!(find_mention_trigger("a#b", 3).is_none());
    }

    /// 验证 skill 过滤按名称与描述匹配。
    #[test]
    fn filters_skills_by_name_and_description() {
        let skills = vec![
            ("drawio".to_string(), "draw diagrams".to_string()),
            ("research".to_string(), "web search notes".to_string()),
        ];
        let trigger = MentionTrigger {
            kind: MentionKind::Skill,
            start: 0,
            end: 4,
            query: "dra".to_string(),
        };
        let items = mention_suggestions(&trigger, &skills);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].insert, "#drawio");
    }

    /// 验证选中项只替换触发片段并移动光标。
    #[test]
    fn applies_mention_without_touching_surrounding_text() {
        let trigger = MentionTrigger {
            kind: MentionKind::Skill,
            start: 2,
            end: 6,
            query: "dra".to_string(),
        };
        let item = MentionSuggestion {
            insert: "#drawio".to_string(),
            label: "#drawio".to_string(),
            description: String::new(),
            continue_filter: false,
        };
        let (next, cursor) = apply_mention("用 #dra 画", &trigger, &item);
        assert_eq!(next, "用 #drawio  画");
        assert_eq!(cursor, "用 #drawio ".chars().count());
    }
}
