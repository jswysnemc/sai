use crate::paths::SaiPaths;
use std::path::PathBuf;

/// 创建命令工具测试使用的隔离路径。
///
/// 参数:
/// - `state_dir`: 测试状态目录
///
/// 返回:
/// - 不访问用户目录的 Sai 路径集合
pub(crate) fn isolated_test_paths(state_dir: PathBuf) -> SaiPaths {
    SaiPaths {
        config_dir: PathBuf::new(),
        config_file: PathBuf::new(),
        secrets_file: PathBuf::new(),
        skills_dir: PathBuf::new(),
        data_dir: PathBuf::new(),
        cache_dir: PathBuf::new(),
        state_dir,
        pictures_dir: PathBuf::new(),
        fish_hook_file: PathBuf::new(),
        bash_hook_file: PathBuf::new(),
        zsh_hook_file: PathBuf::new(),
        powershell_hook_file: PathBuf::new(),
    }
}
