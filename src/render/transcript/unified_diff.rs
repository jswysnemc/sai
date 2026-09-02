/// unified diff 解析出的单文件补丁。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnifiedFilePatch {
    /// 目标文件路径
    pub path: String,
    /// 旧内容行（含上下文）
    pub old_lines: Vec<String>,
    /// 新内容行（含上下文）
    pub new_lines: Vec<String>,
    /// 行级变更序列
    pub lines: Vec<UnifiedLineChange>,
}

/// unified diff 单行变更。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnifiedLineChange {
    /// 变更类型
    pub kind: UnifiedLineKind,
    /// 旧文件行号（上下文与删除行）
    pub old_line: Option<usize>,
    /// 新文件行号（上下文与新增行）
    pub new_line: Option<usize>,
    /// 行文本
    pub text: String,
}

/// unified diff 行类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnifiedLineKind {
    /// 上下文行
    Context,
    /// 新增行
    Add,
    /// 删除行
    Delete,
}

/// 解析 unified diff 文本。
///
/// 只取行级增删与上下文，忽略 hunk 头里的行数：渲染时需要的行号已按
/// 前缀累加推导，文件头（diff --git / --- / +++）仅用于提取展示路径。
///
/// 参数:
/// - `text`: 工具结果里的 unified diff 文本
///
/// 返回:
/// - 解析出的文件补丁；文本不符合 unified diff 结构时返回空
pub(crate) fn parse_unified_diff(text: &str) -> Option<UnifiedFilePatch> {
    let path = text
        .lines()
        .find_map(|line| line.strip_prefix("+++ b/"))
        .map(str::to_string)?;
    let mut patch = UnifiedFilePatch {
        path,
        old_lines: Vec::new(),
        new_lines: Vec::new(),
        lines: Vec::new(),
    };
    let mut old_cursor = 0usize;
    let mut new_cursor = 0usize;
    let mut in_hunk = false;
    for line in text.lines() {
        if let Some(header) = line.strip_prefix("@@ ") {
            let Some((old_start, new_start)) = parse_hunk_starts(header) else {
                return None;
            };
            old_cursor = old_start;
            new_cursor = new_start;
            in_hunk = true;
            continue;
        }
        if !in_hunk {
            continue;
        }
        let (kind, text) = match line.chars().next() {
            Some('+') => (UnifiedLineKind::Add, &line[1..]),
            Some('-') => (UnifiedLineKind::Delete, &line[1..]),
            Some(' ') => (UnifiedLineKind::Context, &line[1..]),
            _ => continue,
        };
        let old_line = matches!(kind, UnifiedLineKind::Context | UnifiedLineKind::Delete)
            .then_some(old_cursor);
        let new_line =
            matches!(kind, UnifiedLineKind::Context | UnifiedLineKind::Add).then_some(new_cursor);
        if matches!(kind, UnifiedLineKind::Context | UnifiedLineKind::Delete) {
            patch.old_lines.push(text.to_string());
            old_cursor += 1;
        }
        if matches!(kind, UnifiedLineKind::Context | UnifiedLineKind::Add) {
            patch.new_lines.push(text.to_string());
            new_cursor += 1;
        }
        patch.lines.push(UnifiedLineChange {
            kind,
            old_line,
            new_line,
            text: text.to_string(),
        });
    }
    (!patch.lines.is_empty()).then_some(patch)
}

/// 解析 hunk 头里的旧行起点与行起点。
///
/// 参数:
/// - `header`: `@@ ` 之后、` @@` 之前的文本，形如 `-1,3 +1,4`
///
/// 返回:
/// - `(旧起点, 新起点)`；结构不符时返回空
fn parse_hunk_starts(header: &str) -> Option<(usize, usize)> {
    let header = header.trim_end().trim_end_matches('@').trim_end();
    let mut parts = header.split_whitespace();
    let old = parts.next()?.strip_prefix('-')?;
    let new = parts.next()?.strip_prefix('+')?;
    let old_start = old.split(',').next()?.parse::<usize>().ok()?;
    let new_start = new.split(',').next()?.parse::<usize>().ok()?;
    // 0 起点 unified diff 表示空侧，渲染时统一按 1 起点推导
    Some((old_start.max(1), new_start.max(1)))
}
