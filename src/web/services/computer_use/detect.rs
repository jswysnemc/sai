//! 解析本机 cua-driver 可执行文件路径。

use std::path::{Path, PathBuf};

/// 【桌面操作】【路径解析】按配置路径、PATH 与安装目录查找驱动。
///
/// 参数:
/// - `configured`: 用户配置的路径或文件名
/// - `install_dir`: Sai 数据目录下的安装根目录
///
/// 返回:
/// - 找到的文件路径
pub(crate) fn resolve_driver_path(configured: &str, install_dir: &Path) -> Option<PathBuf> {
    let configured = configured.trim();
    // 1. 绝对路径或带分隔符的值按文件路径打开
    if looks_like_path(configured) {
        let path = PathBuf::from(configured);
        if path.is_file() {
            return Some(path);
        }
        return None;
    }
    let name = if configured.is_empty() {
        binary_name()
    } else {
        configured
    };
    // 2. 先查 PATH，再回落到 Sai 数据目录中的安装结果
    if let Some(found) = which(name) {
        return Some(found);
    }
    find_driver_binary(install_dir)
}

/// 【桌面操作】【安装文件名】返回当前平台的驱动文件名。
///
/// 参数:
/// - 无
///
/// 返回:
/// - `cua-driver` 或 `cua-driver.exe`
pub(crate) fn binary_name() -> &'static str {
    if cfg!(windows) {
        "cua-driver.exe"
    } else {
        "cua-driver"
    }
}

/// 【桌面操作】【路径判断】绝对路径或带分隔符的值按文件路径处理。
///
/// 参数:
/// - `value`: 配置值
///
/// 返回:
/// - 应按路径打开时为真
fn looks_like_path(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    Path::new(value).is_absolute()
        || value.contains('/')
        || value.contains('\\')
        || value.starts_with('.')
}

/// 【桌面操作】【PATH 查找】在 PATH 中查找可执行文件，不经过 Shell。
///
/// 参数:
/// - `name`: 文件名
///
/// 返回:
/// - 找到的路径
pub(crate) fn which(name: &str) -> Option<PathBuf> {
    let names = candidate_names(name);
    let search = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&search) {
        for candidate in &names {
            let path = directory.join(candidate);
            if path.is_file() {
                return Some(path);
            }
        }
    }
    None
}

/// 【桌面操作】【候选文件名】Windows 下为无扩展名补 `.exe`。
///
/// 参数:
/// - `name`: 原始名称
///
/// 返回:
/// - 待查找文件名
fn candidate_names(name: &str) -> Vec<String> {
    let mut names = vec![name.to_string()];
    if cfg!(windows) && !name.to_ascii_lowercase().ends_with(".exe") {
        names.push(format!("{name}.exe"));
    }
    names
}

/// 【桌面操作】【安装目录扫描】在安装目录中查找驱动二进制。
///
/// 参数:
/// - `root`: 安装根目录
///
/// 返回:
/// - 最短匹配路径
pub(crate) fn find_driver_binary(root: &Path) -> Option<PathBuf> {
    let mut matches = Vec::new();
    walk(root, 0, &mut matches);
    matches.sort_by_key(|path| path.as_os_str().len());
    matches.into_iter().next()
}

/// 【桌面操作】【目录遍历】收集名为 cua-driver 的普通文件。
///
/// 参数:
/// - `directory`: 当前目录
/// - `depth`: 当前深度
/// - `matches`: 匹配结果
///
/// 返回:
/// - 无
fn walk(directory: &Path, depth: usize, matches: &mut Vec<PathBuf>) {
    if depth > 6 || !directory.is_dir() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            walk(&path, depth + 1, matches);
            continue;
        }
        if name == "cua-driver" || name == "cua-driver.exe" {
            matches.push(path);
        }
    }
}
