use crate::paths::SaiPaths;
use std::path::PathBuf;

pub(super) fn config_relative_path(paths: &SaiPaths, value: &str) -> PathBuf {
    let path = PathBuf::from(value.trim());
    if path.is_absolute() {
        path
    } else {
        paths.config_dir.join(path)
    }
}
