use std::path::{Path, PathBuf};

/// 规范化文件系统路径，并在 Windows 上移除扩展路径前缀。
///
/// 参数:
/// - `path`: 待规范化路径
///
/// 返回:
/// - 规范化路径
pub(crate) fn canonicalize(path: &Path) -> std::io::Result<PathBuf> {
    dunce::canonicalize(path)
}

/// 返回适合界面展示和子进程使用的路径。
///
/// 参数:
/// - `path`: 原始路径
///
/// 返回:
/// - 已移除 Windows 扩展路径前缀的路径
pub(crate) fn simplified(path: &Path) -> PathBuf {
    dunce::simplified(path).to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证普通路径保持不变。
    #[test]
    fn keeps_normal_paths_readable() {
        assert_eq!(
            simplified(Path::new("project/src")),
            PathBuf::from("project/src")
        );
    }
}
