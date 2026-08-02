use std::path::PathBuf;

/// patch 文件变更类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FileChange {
    Add {
        path: PathBuf,
        content: String,
    },
    Delete {
        path: PathBuf,
        content: String,
    },
    Update {
        path: PathBuf,
        move_path: Option<PathBuf>,
        new_content: String,
        lines: Vec<LineChange>,
    },
}

impl FileChange {
    /// 返回变更展示路径。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 当前变更主路径
    pub(crate) fn path(&self) -> &PathBuf {
        match self {
            Self::Add { path, .. } | Self::Delete { path, .. } | Self::Update { path, .. } => path,
        }
    }

    /// 返回变更语义标签。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - Added、Deleted、Edited 或 Renamed
    pub(crate) fn action_label(&self) -> &'static str {
        match self {
            Self::Add { .. } => "Added",
            Self::Delete { .. } => "Deleted",
            Self::Update {
                move_path: Some(_), ..
            } => "Renamed",
            Self::Update { .. } => "Edited",
        }
    }

    /// 统计新增和删除行数。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - `(新增行数, 删除行数)`
    pub(crate) fn line_counts(&self) -> (usize, usize) {
        match self {
            Self::Add { content, .. } => (line_count(content), 0),
            Self::Delete { content, .. } => (0, line_count(content)),
            Self::Update { lines, .. } => {
                let added = lines
                    .iter()
                    .filter(|line| line.kind == LineChangeKind::Add)
                    .count();
                let removed = lines
                    .iter()
                    .filter(|line| line.kind == LineChangeKind::Delete)
                    .count();
                (added, removed)
            }
        }
    }
}

/// 已解析并预览完成的 patch。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AppliedPatch {
    pub changes: Vec<FileChange>,
}

impl AppliedPatch {
    /// 判断 patch 是否没有实际文件变更。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 是否为空 patch
    pub(crate) fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }
}

/// 单行 diff 变更。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LineChange {
    pub kind: LineChangeKind,
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
    pub text: String,
}

/// 单行 diff 类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineChangeKind {
    Context,
    Add,
    Delete,
}

/// 统计文本行数。
///
/// 参数:
/// - `content`: 文本内容
///
/// 返回:
/// - `lines()` 口径的行数
fn line_count(content: &str) -> usize {
    content.lines().count()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedPatch {
    pub changes: Vec<ParsedFileChange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ParsedFileChange {
    Add {
        path: PathBuf,
        content: String,
    },
    Delete {
        path: PathBuf,
    },
    Update {
        path: PathBuf,
        move_path: Option<PathBuf>,
        hunks: Vec<PatchHunk>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PatchHunk {
    pub lines: Vec<PatchLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PatchLine {
    Context(String),
    Add(String),
    Delete(String),
}
