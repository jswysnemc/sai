//! 来源读取先验证普通文件，再有界读取；Unix 拒绝链接并避免 FIFO 打开阻塞。
use anyhow::Result;
use std::{io::Read, path::Path};

/// 【Hooks】【安全读取】参数为文件和字节上限，返回不可变快照，拒绝符号链接、特殊文件和超限输入
pub(super) fn read(file: &Path, limit: usize) -> Result<Vec<u8>> {
    let metadata = std::fs::symlink_metadata(file)?;
    anyhow::ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "hook source must be a regular non-symlink file"
    );
    anyhow::ensure!(
        metadata.len() <= limit as u64,
        "hook source exceeds its byte limit"
    );
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(file)?;
    anyhow::ensure!(
        file.metadata()?.is_file(),
        "hook source changed to a special file"
    );
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    anyhow::ensure!(bytes.len() <= limit, "hook source exceeds its byte limit");
    Ok(bytes)
}
