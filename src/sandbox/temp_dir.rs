use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// 沙箱私有临时目录的父目录名。
const ROOT_NAME: &str = "sai-sandbox";

/// 【沙箱】【临时目录】返回本进程专用的可写临时目录，首次调用时创建并清理残留。
///
/// 目录布局为 `$TMPDIR/sai-sandbox-<user>/<pid>`，父目录与本目录权限均为 0700；
/// 命令通过 `TMPDIR` 使用它，避免把整个 `/tmp` 暴露为可写。
///
/// 返回:
/// - 临时目录；创建失败时为 `None`
pub(crate) fn session_temp_dir() -> Option<PathBuf> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        let root = std::env::temp_dir().join(format!("{ROOT_NAME}-{}", user_tag()));
        // 1. 建立用户私有父目录
        create_private_dir(&root).ok()?;
        // 2. 清理已退出进程留下的目录
        remove_stale_dirs(&root);
        // 3. 建立本进程目录
        let dir = root.join(std::process::id().to_string());
        create_private_dir(&dir).ok()?;
        Some(dir.canonicalize().unwrap_or(dir))
    })
    .clone()
}

/// 返回区分用户的目录后缀，避免多用户共享 `/tmp` 时互相冲突。
fn user_tag() -> String {
    #[cfg(unix)]
    {
        // SAFETY: getuid 无副作用且总能成功
        unsafe { libc::getuid() }.to_string()
    }
    #[cfg(not(unix))]
    {
        std::env::var("USERNAME").unwrap_or_else(|_| "user".to_string())
    }
}

/// 创建权限为 0700 的目录；已存在时收紧权限。
///
/// 参数:
/// - `dir`: 目标目录
///
/// 返回:
/// - 创建结果
fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// 删除进程号已不存在的子目录。
///
/// 参数:
/// - `root`: 用户私有父目录
///
/// 返回:
/// - 无；单个目录删除失败时跳过
fn remove_stale_dirs(root: &Path) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        if pid != std::process::id() && !crate::runner::process_liveness::process_exists(pid) {
            // symlink_metadata 保证不会顺着符号链接删除目录外内容
            if entry
                .path()
                .symlink_metadata()
                .is_ok_and(|meta| meta.is_dir())
            {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证临时目录可写且仅当前用户可访问。
    #[test]
    fn session_temp_dir_is_private_and_writable() {
        let dir = session_temp_dir().expect("temp dir");
        std::fs::write(dir.join("probe"), "x").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o700);
        }
        let _ = std::fs::remove_file(dir.join("probe"));
    }

    /// 验证残留目录在进程退出后被清理，存活进程目录保留。
    #[test]
    fn stale_dirs_are_removed() {
        let root = tempfile::tempdir().unwrap();
        let stale = root.path().join("4194303");
        let alive = root.path().join(std::process::id().to_string());
        std::fs::create_dir_all(&stale).unwrap();
        std::fs::create_dir_all(&alive).unwrap();
        remove_stale_dirs(root.path());
        assert!(!stale.exists());
        assert!(alive.exists());
    }
}
