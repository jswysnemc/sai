use std::path::{Path, PathBuf};

/// 展开工具参数中的路径文本。
///
/// 支持 `~/` 家目录前缀；相对路径按当前运行工作目录拼接成绝对路径，
/// 使工具在子智能体、工作区隔离等场景下都落在正确的目录。
///
/// 参数:
/// - `value`: 原始路径文本
///
/// 返回:
/// - 展开后的绝对路径；无法确定工作目录时退回当前目录相对路径
pub(crate) fn expand_path(value: &str) -> PathBuf {
    let value = value.trim();
    // 1. 家目录前缀优先展开
    if let Some(rest) = value.strip_prefix("~/") {
        if let Some(home) = directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf()) {
            return home.join(rest);
        }
    }
    let path = Path::new(value);
    // 2. 绝对路径直接使用，相对路径与运行期工作目录拼接
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        crate::runtime_cwd::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

/// 把文件系统错误转换成带展开后路径的工具错误。
///
/// `std::io::Error` 的文本不含路径，直接冒泡时模型只能看到
/// "No such file or directory (os error 2)"，既无法确认相对路径拼接到了哪里，
/// 也分不清是路径写错还是目标确实不存在。
///
/// 参数:
/// - `action`: 操作描述，例如 "read file"
/// - `path`: 已展开的绝对路径
/// - `error`: 原始文件系统错误
///
/// 返回:
/// - 含完整路径与失败原因的错误
pub(crate) fn fs_error(action: &str, path: &Path, error: &std::io::Error) -> anyhow::Error {
    if error.kind() == std::io::ErrorKind::NotFound {
        return anyhow::anyhow!(
            "{action} failed, path does not exist: {}{}",
            path.display(),
            nearest_existing_ancestor_hint(path)
        );
    }
    anyhow::anyhow!("{action} failed: {} ({error})", path.display())
}

/// 回溯出路径上最近一层已存在的目录。
///
/// 模型给出的路径经常只错中间某一层，指出最近的存在目录能让它一次定位到错误层级，
/// 而不是反复重试整条路径。
///
/// 参数:
/// - `path`: 不存在的目标路径
///
/// 返回:
/// - 可直接拼接到错误文本的提示；没有任何祖先存在时为空串
fn nearest_existing_ancestor_hint(path: &Path) -> String {
    let mut current = path.parent();
    while let Some(ancestor) = current {
        if ancestor.is_dir() {
            // 父目录就存在时说明只是文件名或末级目录名不对，无需额外提示
            if Some(ancestor) == path.parent() {
                return String::new();
            }
            return format!(" (nearest existing directory: {})", ancestor.display());
        }
        current = ancestor.parent();
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证家目录前缀展开为真实家目录。
    #[test]
    fn expands_home_prefix() {
        let Some(home) = directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf())
        else {
            return;
        };

        assert_eq!(expand_path("~/sample.txt"), home.join("sample.txt"));
    }

    /// 验证相对路径拼接到运行期工作目录。
    #[tokio::test]
    async fn joins_relative_path_with_runtime_directory() {
        let workspace = std::path::PathBuf::from("/tmp/sai-expand-path");
        let expanded =
            crate::runtime_cwd::scope(workspace.clone(), async { expand_path("src/main.rs") })
                .await;

        assert_eq!(expanded, workspace.join("src/main.rs"));
    }

    /// 验证绝对路径原样保留。
    #[test]
    fn keeps_absolute_path() {
        let path = std::env::temp_dir().join("sai-absolute.txt");
        let input = path.to_string_lossy().into_owned();

        assert_eq!(expand_path(&input), path);
    }

    /// 验证文件缺失错误带上展开后的完整路径。
    #[test]
    fn missing_file_error_reports_expanded_path() {
        let path = std::env::temp_dir().join("sai-missing-file-xyz.txt");
        let error = std::io::Error::from(std::io::ErrorKind::NotFound);

        let message = fs_error("read file", &path, &error).to_string();

        assert!(message.contains(&path.display().to_string()));
        assert!(message.contains("does not exist"));
        assert!(!message.contains("os error"));
    }

    /// 验证中间目录写错时提示最近的存在目录。
    #[test]
    fn missing_intermediate_directory_reports_nearest_ancestor() {
        let root = tempfile::tempdir().unwrap();
        let path = root
            .path()
            .join("sai-missing-dir-xyz")
            .join("nested")
            .join("sample.txt");
        let error = std::io::Error::from(std::io::ErrorKind::NotFound);

        let message = fs_error("read file", &path, &error).to_string();

        assert!(message.contains(&format!(
            "nearest existing directory: {}",
            root.path().display()
        )));
    }

    /// 验证非缺失类错误保留原始原因。
    #[test]
    fn other_errors_keep_original_reason() {
        let path = std::env::temp_dir().join("sai-denied.txt");
        let error = std::io::Error::from(std::io::ErrorKind::PermissionDenied);

        let message = fs_error("open file", &path, &error).to_string();

        assert!(message.contains(&path.display().to_string()));
        assert!(message.contains("permission denied"));
    }
}
