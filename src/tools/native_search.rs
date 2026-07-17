use anyhow::{Context, Result};
use globset::{GlobBuilder, GlobMatcher};
use regex::Regex;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::{Duration, Instant};

const SEARCH_TIMEOUT: Duration = Duration::from_secs(30);

/// 内置搜索结果。
pub(crate) struct NativeSearchResult {
    pub(crate) lines: Vec<String>,
    pub(crate) truncated: bool,
}

/// 递归查找匹配文件名或相对路径的文件。
///
/// 参数:
/// - `root`: 搜索根目录
/// - `pattern`: 大小写不敏感 Glob 模式
/// - `max_results`: 最大结果数
///
/// 返回:
/// - 匹配文件列表
pub(crate) fn glob_files(
    root: &Path,
    pattern: &str,
    max_results: usize,
) -> Result<NativeSearchResult> {
    let matcher = glob_matcher(pattern)?;
    let deadline = Instant::now() + SEARCH_TIMEOUT;
    let mut lines = Vec::new();
    // 1. 遍历普通文件并匹配相对路径或文件名
    let timed_out = visit_files(root, deadline, &mut |path| {
        let relative = relative_display(root, path);
        if glob_matches(&matcher, &relative, path) {
            lines.push(relative);
        }
        lines.len() > max_results
    })?;
    // 2. 保留最大结果数并记录超时或截断状态
    Ok(limit_results(lines, max_results, timed_out))
}

/// 递归搜索文本文件内容。
///
/// 参数:
/// - `root`: 搜索根目录
/// - `single_file`: 仅搜索指定文件
/// - `pattern`: 正则表达式
/// - `include`: 可选文件 Glob 过滤
/// - `max_results`: 最大结果数
///
/// 返回:
/// - 带文件名和行号的匹配结果
pub(crate) fn grep_text(
    root: &Path,
    single_file: Option<&Path>,
    pattern: &str,
    include: Option<&str>,
    max_results: usize,
) -> Result<NativeSearchResult> {
    let matcher =
        Regex::new(pattern).with_context(|| format!("invalid regex pattern: {pattern}"))?;
    let include = include.map(glob_matcher).transpose()?;
    let deadline = Instant::now() + SEARCH_TIMEOUT;
    let mut lines = Vec::new();
    // 1. 读取指定文件或递归遍历目录中的普通文件
    let timed_out = if let Some(path) = single_file {
        search_file(
            root,
            path,
            &matcher,
            include.as_ref(),
            max_results,
            deadline,
            &mut lines,
        )?
    } else {
        visit_files(root, deadline, &mut |path| {
            let _ = search_file(
                root,
                path,
                &matcher,
                include.as_ref(),
                max_results,
                deadline,
                &mut lines,
            );
            lines.len() > max_results
        })?
    };
    // 2. 保留最大结果数并记录超时或截断状态
    Ok(limit_results(lines, max_results, timed_out))
}

/// 搜索单个文本文件。
///
/// 参数:
/// - `root`: 搜索根目录
/// - `path`: 文件路径
/// - `matcher`: 内容正则表达式
/// - `include`: 可选文件 Glob 过滤器
/// - `max_results`: 最大结果数
/// - `deadline`: 搜索截止时间
/// - `lines`: 匹配结果集合
///
/// 返回:
/// - 是否达到搜索截止时间
fn search_file(
    root: &Path,
    path: &Path,
    matcher: &Regex,
    include: Option<&GlobMatcher>,
    max_results: usize,
    deadline: Instant,
    lines: &mut Vec<String>,
) -> Result<bool> {
    let relative = relative_display(root, path);
    if include.is_some_and(|include| !glob_matches(include, &relative, path)) {
        return Ok(false);
    }
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut buffer = Vec::new();
    let mut line_number = 0usize;
    loop {
        if Instant::now() >= deadline {
            return Ok(true);
        }
        buffer.clear();
        let read = reader.read_until(b'\n', &mut buffer)?;
        if read == 0 || buffer.contains(&0) {
            return Ok(false);
        }
        line_number += 1;
        let line = String::from_utf8_lossy(&buffer);
        let line = line.trim_end_matches(['\r', '\n']);
        if matcher.is_match(line) {
            lines.push(format!("{relative}:{line_number}:{line}"));
            if lines.len() > max_results {
                return Ok(false);
            }
        }
    }
}

/// 递归访问目录中的普通文件，跳过受保护目录。
///
/// 参数:
/// - `root`: 搜索根目录
/// - `deadline`: 搜索截止时间
/// - `visitor`: 文件访问回调；返回真时停止遍历
///
/// 返回:
/// - 是否达到搜索截止时间
fn visit_files(
    root: &Path,
    deadline: Instant,
    visitor: &mut impl FnMut(&Path) -> bool,
) -> Result<bool> {
    std::fs::metadata(root)?;
    let filter_root = root.to_path_buf();
    let mut builder = ignore::WalkBuilder::new(root);
    builder
        .hidden(false)
        .ignore(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .parents(true)
        .follow_links(false)
        .filter_entry(move |entry| !excluded_directory(&filter_root, entry.path()));
    // 1. 使用 ignore 遍历器保持与 ripgrep 一致的忽略规则
    for entry in builder.build() {
        if Instant::now() >= deadline {
            return Ok(true);
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        // 2. 只向调用方传递普通文件
        if entry
            .file_type()
            .is_some_and(|file_type| file_type.is_file())
            && visitor(entry.path())
        {
            return Ok(false);
        }
    }
    Ok(false)
}

/// 创建大小写不敏感的 Glob 匹配器。
///
/// 参数:
/// - `pattern`: Glob 模式
///
/// 返回:
/// - Glob 匹配器
fn glob_matcher(pattern: &str) -> Result<GlobMatcher> {
    GlobBuilder::new(pattern)
        .case_insensitive(true)
        .literal_separator(true)
        .build()
        .map(|glob| glob.compile_matcher())
        .with_context(|| format!("invalid glob pattern: {pattern}"))
}

/// 判断相对路径或文件名是否符合 Glob 模式。
///
/// 参数:
/// - `matcher`: Glob 匹配器
/// - `relative`: 正斜杠格式的相对路径
/// - `path`: 文件路径
///
/// 返回:
/// - 是否匹配
fn glob_matches(matcher: &GlobMatcher, relative: &str, path: &Path) -> bool {
    matcher.is_match(relative)
        || path
            .file_name()
            .is_some_and(|name| matcher.is_match(Path::new(name)))
}

/// 判断目录是否属于搜索排除范围。
///
/// 参数:
/// - `root`: 搜索根目录
/// - `directory`: 待判断目录
///
/// 返回:
/// - 是否应跳过目录
fn excluded_directory(root: &Path, directory: &Path) -> bool {
    if directory.file_name().is_some_and(|name| name == ".git") {
        return true;
    }
    if root != Path::new("/") {
        return false;
    }
    let relative = relative_display(root, directory);
    matches!(
        relative.as_str(),
        "dev" | "proc" | "sys" | "run" | "tmp" | "usr" | "nix" | "snap" | "flatpak"
    ) || relative.starts_with("var/cache")
        || relative.starts_with("var/lib")
        || relative.starts_with("var/log")
}

/// 返回相对搜索根目录的跨平台显示路径。
///
/// 参数:
/// - `root`: 搜索根目录
/// - `path`: 文件绝对路径
///
/// 返回:
/// - 使用正斜杠分隔的相对路径
fn relative_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// 限制搜索结果数量。
///
/// 参数:
/// - `lines`: 全部匹配结果
/// - `max_results`: 最大结果数
/// - `timed_out`: 是否达到搜索截止时间
///
/// 返回:
/// - 截断后的搜索结果
fn limit_results(
    mut lines: Vec<String>,
    max_results: usize,
    timed_out: bool,
) -> NativeSearchResult {
    let truncated = timed_out || lines.len() > max_results;
    lines.truncate(max_results);
    NativeSearchResult { lines, truncated }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证 Glob 支持递归模式和字符组。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn glob_supports_recursive_paths_and_character_classes() {
        let temp = tempfile::tempdir().unwrap();
        let nested = temp.path().join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("file1.rs"), "content").unwrap();

        let result = glob_files(temp.path(), "**/file[0-9].rs", 10).unwrap();

        assert_eq!(result.lines, vec!["nested/file1.rs"]);
    }

    /// 验证原生遍历遵守 Git 忽略规则。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn glob_respects_gitignore_rules() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join(".git")).unwrap();
        std::fs::write(temp.path().join(".gitignore"), "ignored/\n").unwrap();
        std::fs::create_dir_all(temp.path().join("ignored")).unwrap();
        std::fs::write(temp.path().join("ignored/file.rs"), "content").unwrap();
        std::fs::write(temp.path().join("visible.rs"), "content").unwrap();

        let result = glob_files(temp.path(), "**/*.rs", 10).unwrap();

        assert_eq!(result.lines, vec!["visible.rs"]);
    }
}
