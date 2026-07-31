use std::path::{Path, PathBuf};

/// 返回会话索引文件路径。
///
/// 参数:
/// - `base_state_dir`: 原始状态目录
///
/// 返回:
/// - 会话索引文件路径
pub(super) fn sessions_file(base_state_dir: &Path) -> PathBuf {
    base_state_dir.join("index.json")
}

/// 返回当前会话文件路径。
///
/// 参数:
/// - `base_state_dir`: 原始状态目录
///
/// 返回:
/// - 当前会话文件路径
pub(super) fn current_session_file(base_state_dir: &Path) -> PathBuf {
    base_state_dir.join("current")
}
