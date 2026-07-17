use crate::paths::SaiPaths;
use std::path::PathBuf;

/// 返回网关共享工作区目录。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - 网关共享工作区目录
pub(crate) fn gateway_workspace_path(paths: &SaiPaths) -> PathBuf {
    paths.data_dir.join("workspace")
}
