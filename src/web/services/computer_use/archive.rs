//! 从官方归档中安全抽出 cua-driver 二进制及必要辅助文件。

use anyhow::{bail, Context, Result};
use flate2::read::GzDecoder;
use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};

const MAX_FILE_BYTES: u64 = 80 * 1024 * 1024;

/// 【桌面操作】【归档解压】把 tar.gz 或 zip 中的允许文件写到目标目录。
///
/// 参数:
/// - `bytes`: 归档字节
/// - `archive_name`: 归档文件名
/// - `destination`: 解压目录
///
/// 返回:
/// - 驱动二进制路径
pub(crate) fn extract_driver(
    bytes: &[u8],
    archive_name: &str,
    destination: &Path,
) -> Result<PathBuf> {
    if destination.exists() {
        std::fs::remove_dir_all(destination)
            .with_context(|| format!("failed to clear {}", destination.display()))?;
    }
    std::fs::create_dir_all(destination)?;
    // 1. 按扩展名选择 tar.gz 或 zip
    if archive_name.ends_with(".zip") {
        extract_zip(bytes, destination)?;
    } else {
        extract_tar_gz(bytes, destination)?;
    }
    // 2. 安装目录中必须能找到 cua-driver 二进制
    super::detect::find_driver_binary(destination)
        .ok_or_else(|| anyhow::anyhow!("archive does not contain cua-driver"))
}

/// 【桌面操作】【tar 解压】只写出允许的普通文件，拒绝绝对路径和父目录跳转。
///
/// 参数:
/// - `bytes`: gzip 压缩的 tar
/// - `destination`: 解压目录
///
/// 返回:
/// - 解压是否成功
fn extract_tar_gz(bytes: &[u8], destination: &Path) -> Result<()> {
    let decoder = GzDecoder::new(Cursor::new(bytes));
    let mut archive = tar::Archive::new(decoder);
    for entry in archive.entries()? {
        let mut entry = entry?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let source = entry.path()?.into_owned();
        let Some(relative) = allowed_rel(&source) else {
            continue;
        };
        let size = entry.header().size()?;
        if size > MAX_FILE_BYTES {
            bail!("{} exceeds the 80 MiB extract limit", relative.display());
        }
        let mut data = Vec::new();
        entry.read_to_end(&mut data)?;
        write_file(destination, &relative, &data)?;
    }
    Ok(())
}

/// 【桌面操作】【zip 解压】只写出允许的普通文件。
///
/// 参数:
/// - `bytes`: zip 字节
/// - `destination`: 解压目录
///
/// 返回:
/// - 解压是否成功
fn extract_zip(bytes: &[u8], destination: &Path) -> Result<()> {
    let cursor = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).context("invalid zip archive")?;
    for index in 0..archive.len() {
        let mut file = archive.by_index(index)?;
        if file.is_dir() {
            continue;
        }
        let source = PathBuf::from(file.name());
        let Some(relative) = allowed_rel(&source) else {
            continue;
        };
        if file.size() > MAX_FILE_BYTES {
            bail!("{} exceeds the 80 MiB extract limit", relative.display());
        }
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        write_file(destination, &relative, &data)?;
    }
    Ok(())
}

/// 【桌面操作】【路径过滤】去掉包目录前缀，并拒绝不安全或无关文件。
///
/// 参数:
/// - `path`: 归档内路径
///
/// 返回:
/// - 允许写出的相对路径
fn allowed_rel(path: &Path) -> Option<PathBuf> {
    let stripped = strip_package_prefix(path);
    let relative = safe_rel(&stripped)?;
    let name = relative.file_name()?.to_string_lossy();
    let allowed_name = matches!(
        name.as_ref(),
        "cua-driver" | "cua-driver.exe" | "cua-driver-uia.exe" | "cua-cursor-theme"
    );
    let helper = relative
        .components()
        .any(|component| component.as_os_str() == "wayland-helper");
    if allowed_name || helper {
        Some(relative)
    } else {
        None
    }
}

/// 【桌面操作】【包前缀】去掉官方归档常见的顶层目录。
///
/// 参数:
/// - `path`: 归档内路径
///
/// 返回:
/// - 去掉顶层目录后的路径
fn strip_package_prefix(path: &Path) -> PathBuf {
    let mut components = path.components();
    let Some(first) = components.next() else {
        return PathBuf::new();
    };
    let rest: PathBuf = components.collect();
    if rest.as_os_str().is_empty() {
        return PathBuf::from(first.as_os_str());
    }
    let first_name = first.as_os_str().to_string_lossy();
    if first_name.starts_with("cua-driver") {
        rest
    } else {
        path.to_path_buf()
    }
}

/// 【桌面操作】【安全相对路径】拒绝绝对路径、盘符和 `..`。
///
/// 参数:
/// - `path`: 候选路径
///
/// 返回:
/// - 仅含普通分量的相对路径
fn safe_rel(path: &Path) -> Option<PathBuf> {
    if path.is_absolute() {
        return None;
    }
    let mut relative = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => relative.push(part),
            Component::CurDir => {}
            _ => return None,
        }
    }
    if relative.as_os_str().is_empty() {
        None
    } else {
        Some(relative)
    }
}

/// 【桌面操作】【文件写出】创建父目录并写入内容，Unix 下给驱动加执行权限。
///
/// 参数:
/// - `root`: 解压根目录
/// - `relative`: 相对路径
/// - `data`: 文件内容
///
/// 返回:
/// - 写出是否成功
fn write_file(root: &Path, relative: &Path, data: &[u8]) -> Result<()> {
    let target = root.join(relative);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&target, data)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let name = relative
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        if matches!(
            name,
            "cua-driver" | "cua-driver.exe" | "cua-driver-uia.exe" | "cua-cursor-theme"
        ) {
            std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755))?;
        }
    }
    Ok(())
}
