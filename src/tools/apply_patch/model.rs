//! 补丁协议模型与资源上限，不执行文件系统操作。

pub(super) const MAX_PATCH_BYTES: usize = 1024 * 1024;
pub(super) const MAX_FILES: usize = 64;
pub(super) const MAX_FILE_BYTES: usize = 8 * 1024 * 1024;
pub(super) const MAX_TOTAL_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Operation {
    Add {
        path: String,
        content: String,
    },
    Delete {
        path: String,
    },
    Update {
        path: String,
        destination: Option<String>,
        hunks: Vec<Hunk>,
    },
}

impl Operation {
    /// 【文件补丁】【源路径】获取协议中的源文件路径，不执行展开。
    /// 参数: 无；返回: 路径文本。
    pub(super) fn path(&self) -> &str {
        match self {
            Self::Add { path, .. } | Self::Delete { path } | Self::Update { path, .. } => path,
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Hunk {
    pub anchor: Option<String>,
    pub old: Vec<String>,
    pub new: Vec<String>,
    pub at_eof: bool,
    pub old_no_newline: bool,
    pub new_no_newline: bool,
}
