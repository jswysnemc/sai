use std::io;

/// 【插件文件锁】【创建竞争】打开协调文件，处理并发首次创建时短暂缺失的目录项。
/// @param operation 绑定原目录句柄及打开选项的操作
/// @returns 原始文件句柄或不可恢复的打开错误
pub(super) fn open<T>(mut operation: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    let mut retries = 7;
    loop {
        match operation() {
            Err(error) if error.kind() == io::ErrorKind::NotFound && retries > 0 => {
                // 1. 【插件文件锁】【创建竞争】macOS 并发创建期间可能短暂返回 ENOENT，等待目录项可见
                retries -= 1;
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            result => return result,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    /// 【插件文件锁】【首次创建】其他调用已创建文件时，短暂 ENOENT 之后重新打开并保留原内容。
    /// @returns 无，打开失败或已有内容被破坏时断言失败
    #[test]
    fn transient_missing_entry_reopens_the_winning_file_without_truncation() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("coordination.lock");
        let mut attempts = 0;
        let mut file = open(|| {
            attempts += 1;
            if attempts == 1 {
                std::fs::write(&path, "existing coordination data")?;
                return Err(io::ErrorKind::NotFound.into());
            }
            std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(&path)
        })
        .unwrap();
        let mut content = String::new();
        file.read_to_string(&mut content).unwrap();
        assert_eq!(attempts, 2);
        assert_eq!(content, "existing coordination data");
    }

    /// 【插件文件锁】【持续错误】目录持续缺失时，达到尝试上限后返回原错误。
    /// @returns 无，不得无限循环
    #[test]
    fn permanent_missing_entry_returns_after_a_bounded_number_of_attempts() {
        let mut attempts = 0;
        let error = open::<()>(|| {
            attempts += 1;
            Err(io::ErrorKind::NotFound.into())
        })
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert_eq!(attempts, 8);
    }

    /// 【插件文件锁】【错误边界】权限、路径类型及锁占用错误必须立即向上传递。
    /// @returns 无，仅短暂缺失错误允许再次打开
    #[test]
    fn other_errors_are_not_retried() {
        for kind in [
            io::ErrorKind::PermissionDenied,
            io::ErrorKind::NotADirectory,
            io::ErrorKind::AlreadyExists,
            io::ErrorKind::WouldBlock,
        ] {
            let mut attempts = 0;
            let error = open::<()>(|| {
                attempts += 1;
                Err(kind.into())
            })
            .unwrap_err();
            assert_eq!(error.kind(), kind);
            assert_eq!(attempts, 1);
        }
    }
}
