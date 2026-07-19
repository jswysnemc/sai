use serde_json::Value;
use std::path::{Component, Path, PathBuf};

/// 不访问文件系统地规范化工作区相对路径。
///
/// 参数:
/// - `workspace`: 工作区根目录
/// - `path`: 待解析路径
///
/// 返回:
/// - 消除当前目录和父目录段后的路径
pub(super) fn resolve_without_io(workspace: &Path, path: &Path) -> PathBuf {
    let mut output = if path.is_absolute() {
        PathBuf::new()
    } else {
        workspace.to_path_buf()
    };
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => output.push(prefix.as_os_str()),
            Component::RootDir => output.push(Path::new("/")),
            Component::CurDir => {}
            Component::ParentDir => {
                output.pop();
            }
            Component::Normal(value) => output.push(value),
        }
    }
    output
}

/// 校验目标路径及其最近存在祖先没有通过符号链接逃逸工作区。
///
/// 参数:
/// - `workspace`: 工作区根目录
/// - `path`: 待校验路径
///
/// 返回:
/// - 路径是否位于工作区
pub(super) fn path_is_within_workspace(workspace: &Path, path: &Path) -> bool {
    if !path.starts_with(workspace) {
        return false;
    }
    // 工作区尚未创建时无法做符号链接解析，保留词法边界判断。
    if !workspace.exists() {
        return true;
    }
    let mut ancestor = path;
    while !ancestor.exists() {
        let Some(parent) = ancestor.parent() else {
            return false;
        };
        ancestor = parent;
    }
    ancestor
        .canonicalize()
        .map(|canonical| canonical.starts_with(workspace))
        .unwrap_or(false)
}

/// 判断读取参数中是否包含受保护系统路径。
///
/// 参数:
/// - `workspace`: 当前会话工作区
/// - `arguments`: 工具参数
///
/// 返回:
/// - 任一路径位于敏感系统目录时返回 `true`
pub(super) fn contains_sensitive_read_path(workspace: &Path, arguments: &Value) -> bool {
    for key in ["path", "file", "target", "destination", "cwd"] {
        if arguments
            .get(key)
            .and_then(Value::as_str)
            .is_some_and(|value| is_sensitive_path(workspace, Path::new(value)))
        {
            return true;
        }
    }
    for key in ["files", "paths"] {
        let Some(values) = arguments.get(key).and_then(Value::as_array) else {
            continue;
        };
        if values.iter().any(|value| {
            let path = value
                .as_str()
                .or_else(|| value.get("path").and_then(Value::as_str));
            path.is_some_and(|value| is_sensitive_path(workspace, Path::new(value)))
        }) {
            return true;
        }
    }
    false
}

/// 判断工具参数中是否包含工作区外路径。
///
/// 参数:
/// - `workspace`: 当前会话工作区
/// - `arguments`: 工具参数
///
/// 返回:
/// - 任一路径解析后位于工作区外时返回 `true`
pub(super) fn contains_external_path(workspace: &Path, arguments: &Value) -> bool {
    for key in ["path", "file", "target", "destination", "cwd"] {
        if arguments
            .get(key)
            .and_then(Value::as_str)
            .is_some_and(|value| path_is_external(workspace, Path::new(value)))
        {
            return true;
        }
    }
    for key in ["files", "paths"] {
        let Some(values) = arguments.get(key).and_then(Value::as_array) else {
            continue;
        };
        if values.iter().any(|value| {
            let path = value
                .as_str()
                .or_else(|| value.get("path").and_then(Value::as_str));
            path.is_some_and(|value| path_is_external(workspace, Path::new(value)))
        }) {
            return true;
        }
    }
    if let Some(patch) = arguments.get("patch").and_then(Value::as_str) {
        for line in patch.lines() {
            let path = if let Some(destination) = line.strip_prefix("*** Move to: ") {
                Some(destination.trim())
            } else {
                line.split_once(" File: ").map(|(_, value)| value)
            };
            let Some(path) = path else {
                continue;
            };
            let source = path
                .split_once(" -> ")
                .map(|(value, _)| value)
                .unwrap_or(path);
            if path_is_external(workspace, Path::new(source.trim())) {
                return true;
            }
            if let Some((_, destination)) = path.split_once(" -> ") {
                if path_is_external(workspace, Path::new(destination.trim())) {
                    return true;
                }
            }
        }
    }
    false
}

/// 判断单一路径是否位于工作区外。
///
/// 参数:
/// - `workspace`: 当前会话工作区
/// - `path`: 待检查路径
///
/// 返回:
/// - 路径位于工作区外时返回 `true`
fn path_is_external(workspace: &Path, path: &Path) -> bool {
    !path_is_within_workspace(workspace, &resolve_user_path(workspace, path))
}

/// 解析支持 `~`、`~/` 和 `~\\` 的用户路径，其他相对路径相对当前工作区解析。
///
/// 参数:
/// - `workspace`: 当前会话工作区
/// - `path`: 待解析路径
///
/// 返回:
/// - 规范化后的路径
fn resolve_user_path(workspace: &Path, path: &Path) -> PathBuf {
    if let Some(value) = path.to_str() {
        let rest = value
            .strip_prefix("~/")
            .or_else(|| value.strip_prefix("~\\"));
        if let Some(dirs) = directories::BaseDirs::new() {
            if value == "~" {
                return dirs.home_dir().to_path_buf();
            }
            if let Some(rest) = rest {
                return dirs.home_dir().join(rest);
            }
        }
    }
    resolve_without_io(workspace, path)
}

/// 判断单一路径是否属于需要确认的系统目录。
///
/// 参数:
/// - `workspace`: 当前会话工作区
/// - `path`: 待检查路径
///
/// 返回:
/// - 路径位于系统敏感目录时返回 `true`
fn is_sensitive_path(workspace: &Path, path: &Path) -> bool {
    let path = resolve_user_path(workspace, path);
    if path_has_sensitive_name(&path) {
        return true;
    }
    if path_is_within_workspace(workspace, &path) {
        return false;
    }
    let mut components = path.components();
    let mut first = components.next();
    if matches!(first, Some(Component::Prefix(_))) {
        first = components.next();
    }
    if !matches!(first, Some(Component::RootDir)) {
        return false;
    }
    let Some(Component::Normal(root)) = components.next() else {
        return false;
    };
    let root = root.to_string_lossy().to_ascii_lowercase();
    matches!(
        root.as_str(),
        "etc"
            | "boot"
            | "dev"
            | "home"
            | "root"
            | "run"
            | "sys"
            | "tmp"
            | "var"
            | "usr"
            | "bin"
            | "sbin"
            | "lib"
            | "lib64"
            | "opt"
            | "windows"
            | "programdata"
            | "program files"
            | "program files (x86)"
            | "users"
    )
}

/// 判断路径是否包含常见凭据文件或目录名称。
///
/// 参数:
/// - `path`: 待检查路径
///
/// 返回:
/// - 路径可能包含凭据时返回 `true`
fn path_has_sensitive_name(path: &Path) -> bool {
    path.components().any(|component| {
        let Component::Normal(value) = component else {
            return false;
        };
        let Some(value) = value.to_str() else {
            return false;
        };
        let value = value.to_ascii_lowercase();
        value == ".ssh"
            || value == ".gnupg"
            || value.starts_with(".env")
            || value.contains("credential")
            || value.contains("secret")
            || matches!(
                Path::new(&value)
                    .extension()
                    .and_then(|extension| extension.to_str()),
                Some("pem" | "key" | "p12" | "pfx")
            )
    })
}
