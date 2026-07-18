use crate::i18n::text as t;
use crate::tools::{ToolRegistry, ToolSpec};
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// 注册移入回收站工具。
///
/// 参数:
/// - `registry`: 工具注册表
pub(crate) fn register(registry: &mut ToolRegistry) {
    registry.register(ToolSpec::new(
        "trash_path",
        t("Move a file, directory, or symlink to the system Trash instead of permanently deleting it. Use this when the user asks to delete/remove/clean up a local path; do not use rm unless explicitly requested. Only claim success when exists_after is false.", "把文件、目录或符号链接移入系统回收站，而不是永久删除。用户要求删除/移除/清理本地路径时优先使用它；除非用户明确要求，不要使用 rm。只有 exists_after 为 false 时才说明已处理。"),
        json!({"type":"object","properties":{"path":{"type":"string","description": t("File, directory, or symlink path to move to Trash. Supports absolute paths, workspace-relative paths, and ~/ paths.", "要移入回收站的文件、目录或符号链接路径。支持绝对路径、工作区相对路径和 ~/ 路径。")}},"required":["path"],"additionalProperties":false}),
        |args| async move { trash_path(args) },
    ).writes());
}

/// 将路径移入系统回收站。
///
/// 参数:
/// - `args`: 工具参数
///
/// 返回:
/// - JSON 格式处理结果
fn trash_path(args: Value) -> Result<String> {
    let input = path_arg(&args, "path")?;
    let resolved = resolve_existing_path_without_following_leaf(&input)?;
    ensure_safe_trash_target(&resolved)?;
    let metadata = std::fs::symlink_metadata(&resolved)?;
    let kind = path_kind(&metadata);
    let existed_before = true;
    let original_path = resolved.display().to_string();
    let timestamp_before = current_unix_seconds();

    trash::delete(&resolved).map_err(|err| anyhow::anyhow!("failed to move to trash: {err}"))?;

    let exists_after = std::fs::symlink_metadata(&resolved).is_ok();
    let trash_item_id = find_recent_trash_item(&resolved, timestamp_before);
    Ok(serde_json::to_string_pretty(&json!({
        "ok": !exists_after,
        "kind": kind,
        "original_path": original_path,
        "existed_before": existed_before,
        "exists_after": exists_after,
        "trash_item_id": trash_item_id,
        "restore_hint": t("Open the system Trash and restore the item if needed.", "如需恢复，请打开系统回收站并还原该项目。"),
        "note": t("The path was moved to Trash, not permanently deleted.", "该路径已移入回收站，并未永久删除。")
    }))?)
}

/// 解析既有路径，但不跟随叶子节点符号链接。
///
/// 参数:
/// - `path`: 输入路径
///
/// 返回:
/// - 解析后的路径
fn resolve_existing_path_without_following_leaf(path: &Path) -> Result<PathBuf> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let filename = path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("refusing to trash a root path: {}", path.display()))?;
    let parent = parent.canonicalize()?;
    let resolved = parent.join(filename);
    std::fs::symlink_metadata(&resolved)?;
    Ok(resolved)
}

/// 校验回收站目标是否安全。
///
/// 参数:
/// - `path`: 待删除路径
///
/// 返回:
/// - 安全时返回成功
fn ensure_safe_trash_target(path: &Path) -> Result<()> {
    let cwd = crate::runtime_cwd::current_dir()?.canonicalize()?;
    let resolved_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let home = directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf());
    if is_dangerous_system_path(path, &resolved_path) {
        bail!(
            "refusing to trash dangerous system path: {}",
            path.display()
        )
    }
    if path == cwd || resolved_path == cwd {
        bail!(
            "refusing to trash current workspace root: {}",
            path.display()
        )
    }
    if let Some(home) = home {
        if path == home {
            bail!("refusing to trash home directory: {}", path.display())
        }
        if let Some(trash_dir) = trash_directory(&home) {
            if path == trash_dir || path.starts_with(&trash_dir) {
                bail!(
                    "refusing to trash the Trash directory itself: {}",
                    path.display()
                )
            }
        }
    }
    Ok(())
}

/// 判断路径是否属于当前平台的系统目录。
///
/// 参数:
/// - `path`: 未跟随叶子符号链接的路径
/// - `resolved_path`: 已尽可能解析的路径
///
/// 返回:
/// - 系统目录或系统根目录返回 `true`
fn is_dangerous_system_path(path: &Path, resolved_path: &Path) -> bool {
    let dangerous = dangerous_system_paths();
    if dangerous
        .iter()
        .any(|item| path == Path::new(item) || resolved_path == Path::new(item))
    {
        return true;
    }
    #[cfg(windows)]
    {
        return windows_dangerous_system_paths()
            .iter()
            .any(|item| path == item || resolved_path == item);
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// 返回当前平台需要保护的系统路径。
///
/// 返回:
/// - 不应移入回收站的系统目录
fn dangerous_system_paths() -> &'static [&'static str] {
    #[cfg(target_os = "macos")]
    {
        return &[
            "/",
            "/Applications",
            "/Library",
            "/System",
            "/Users",
            "/bin",
            "/dev",
            "/etc",
            "/private",
            "/sbin",
            "/tmp",
            "/usr",
            "/var",
        ];
    }
    #[cfg(windows)]
    {
        return &[];
    }
    #[cfg(all(not(target_os = "macos"), not(windows)))]
    {
        &[
            "/", "/bin", "/boot", "/dev", "/etc", "/home", "/opt", "/proc", "/root", "/run",
            "/sbin", "/sys", "/tmp", "/usr", "/var",
        ]
    }
}

/// 返回 Windows 需要保护的系统目录。
///
/// 返回:
/// - 从系统环境变量解析的 Windows、Program Files、ProgramData 和用户根目录
#[cfg(windows)]
fn windows_dangerous_system_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let system_root = std::env::var_os("SystemRoot")
        .or_else(|| std::env::var_os("WINDIR"))
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("SystemDrive").map(|drive| PathBuf::from(drive).join("Windows"))
        });
    if let Some(system_root) = system_root {
        paths.push(system_root.clone());
        if let Some(root) = system_root.parent() {
            paths.push(root.to_path_buf());
            paths.push(root.join("Users"));
            paths.push(root.join("ProgramData"));
            paths.push(root.join("Program Files"));
            paths.push(root.join("Program Files (x86)"));
        }
    }
    for variable in ["ProgramFiles", "ProgramFiles(x86)", "ProgramData"] {
        if let Some(value) = std::env::var_os(variable) {
            paths.push(PathBuf::from(value));
        }
    }
    paths
}

/// 返回当前用户的系统回收站目录。
///
/// 参数:
/// - `home`: 当前用户主目录
///
/// 返回:
/// - 可识别的回收站目录；Windows 由系统 API 管理，不返回路径
fn trash_directory(home: &Path) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        Some(home.join(".Trash"))
    }
    #[cfg(all(not(target_os = "macos"), not(windows)))]
    {
        Some(home.join(".local/share/Trash"))
    }
    #[cfg(windows)]
    {
        let _ = home;
        None
    }
}

/// 获取路径类型。
///
/// 参数:
/// - `metadata`: 文件元数据
///
/// 返回:
/// - 路径类型文本
fn path_kind(metadata: &std::fs::Metadata) -> &'static str {
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        "symlink"
    } else if file_type.is_dir() {
        "directory"
    } else if file_type.is_file() {
        "file"
    } else {
        "other"
    }
}

/// 当前 Unix 时间戳。
///
/// 返回:
/// - 秒级 Unix 时间戳
fn current_unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

/// 查找最近的回收站项目。
///
/// 参数:
/// - `original_path`: 原始路径
/// - `timestamp_before`: 删除前时间戳
///
/// 返回:
/// - 回收站项目 ID
fn find_recent_trash_item(original_path: &Path, timestamp_before: i64) -> Option<String> {
    #[cfg(any(
        target_os = "windows",
        all(
            unix,
            not(target_os = "macos"),
            not(target_os = "ios"),
            not(target_os = "android")
        )
    ))]
    {
        let items = trash::os_limited::list().ok()?;
        items
            .into_iter()
            .filter(|item| item.original_path() == original_path)
            .filter(|item| {
                item.time_deleted < 0 || item.time_deleted >= timestamp_before.saturating_sub(2)
            })
            .max_by_key(|item| item.time_deleted)
            .map(|item| item.id.to_string_lossy().to_string())
    }
    #[cfg(not(any(
        target_os = "windows",
        all(
            unix,
            not(target_os = "macos"),
            not(target_os = "ios"),
            not(target_os = "android")
        )
    )))]
    {
        let _ = original_path;
        let _ = timestamp_before;
        None
    }
}

/// 读取路径参数。
///
/// 参数:
/// - `args`: 工具参数
/// - `key`: 参数名
///
/// 返回:
/// - 路径参数
fn path_arg(args: &Value, key: &str) -> Result<PathBuf> {
    let value = required(args, key)?;
    Ok(expand_path(&value))
}

/// 展开路径。
///
/// 参数:
/// - `value`: 原始路径
///
/// 返回:
/// - 展开后的路径
fn expand_path(value: &str) -> PathBuf {
    let value = value.trim();
    if let Some(rest) = value.strip_prefix("~/") {
        if let Some(home) = directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf()) {
            return home.join(rest);
        }
    }
    let path = Path::new(value);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        crate::runtime_cwd::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

/// 读取必填字符串参数。
///
/// 参数:
/// - `args`: 工具参数
/// - `key`: 参数名
///
/// 返回:
/// - 参数值
fn required(args: &Value, key: &str) -> Result<String> {
    let value = args
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if value.is_empty() {
        bail!("{}: {key}", t("required argument missing", "缺少必需参数"))
    } else {
        Ok(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trash_path_rejects_workspace_root() {
        let cwd = crate::runtime_cwd::current_dir().unwrap();
        assert!(ensure_safe_trash_target(&cwd).is_err());
    }

    #[test]
    fn trash_path_moves_file_to_trash() {
        let cwd = crate::runtime_cwd::current_dir().unwrap();
        let temp = tempfile::tempdir_in(cwd).unwrap();
        let path = temp.path().join("delete-me.txt");
        std::fs::write(&path, "bye").unwrap();
        let result = trash_path(json!({"path": path.display().to_string()})).unwrap();
        let data: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(data["ok"], true);
        assert_eq!(data["exists_after"], false);
    }

    #[test]
    fn trash_path_moves_directory_to_trash() {
        let cwd = crate::runtime_cwd::current_dir().unwrap();
        let temp = tempfile::tempdir_in(cwd).unwrap();
        let path = temp.path().join("delete-dir");
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("file.txt"), "bye").unwrap();
        let result = trash_path(json!({"path": path.display().to_string()})).unwrap();
        let data: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(data["ok"], true);
        assert_eq!(data["exists_after"], false);
    }
}
