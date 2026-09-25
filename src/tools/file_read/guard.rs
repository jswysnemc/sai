use crate::tools::fs_path::fs_error;
use anyhow::{bail, Result};
use std::io::Read;
use std::path::{Path, PathBuf};

/// 读取后会阻塞或产生无限输出的设备文件。
const BLOCKED_DEVICE_PATHS: &[&str] = &[
    "/dev/zero",
    "/dev/random",
    "/dev/urandom",
    "/dev/full",
    "/dev/stdin",
    "/dev/tty",
    "/dev/console",
    "/dev/stdout",
    "/dev/stderr",
    "/dev/fd/0",
    "/dev/fd/1",
    "/dev/fd/2",
];

/// 不能按文本读取的二进制扩展名；图片与 PDF 在分派时单独放行。
const BINARY_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "ico", "webp", "tiff", "tif", "mp4", "mov", "avi", "mkv",
    "webm", "wmv", "flv", "m4v", "mpeg", "mpg", "mp3", "wav", "ogg", "flac", "aac", "m4a", "wma",
    "aiff", "opus", "zip", "tar", "gz", "bz2", "7z", "rar", "xz", "z", "tgz", "iso", "exe", "dll",
    "so", "dylib", "bin", "o", "a", "obj", "lib", "app", "msi", "deb", "rpm", "pdf", "doc", "docx",
    "xls", "xlsx", "ppt", "pptx", "odt", "ods", "odp", "ttf", "otf", "woff", "woff2", "eot", "pyc",
    "pyo", "class", "jar", "war", "ear", "node", "wasm", "rlib", "sqlite", "sqlite3", "db", "mdb",
    "idx", "psd", "ai", "eps", "sketch", "fig", "xd", "blend", "3ds", "max", "swf", "fla", "lockb",
    "dat", "data",
];

/// 文件不存在时附加的工作目录说明。
const FILE_NOT_FOUND_CWD_NOTE: &str = "Note: your current working directory is";

/// 判断路径是否为会阻塞或无限输出的设备文件。
///
/// 参数:
/// - `path`: 已展开路径
///
/// 返回:
/// - 命中黑名单时为 true
pub(super) fn is_blocked_device_path(path: &Path) -> bool {
    let text = path.to_string_lossy();
    if BLOCKED_DEVICE_PATHS.contains(&text.as_ref()) {
        return true;
    }
    text.starts_with("/proc/")
        && (text.ends_with("/fd/0") || text.ends_with("/fd/1") || text.ends_with("/fd/2"))
}

/// 判断扩展名是否属于二进制文件。
///
/// 参数:
/// - `extension`: 小写扩展名，不含点
///
/// 返回:
/// - 属于二进制扩展名时为 true
pub(super) fn has_binary_extension(extension: &str) -> bool {
    BINARY_EXTENSIONS.contains(&extension)
}

/// 抽样检查文件内容是否像二进制数据。
///
/// 参数:
/// - `path`: 文件路径
///
/// 返回:
/// - 可以按文本读取时成功；包含 NUL 或控制字符比例过高时报错
pub(super) fn ensure_text_content(path: &Path) -> Result<()> {
    let mut file =
        std::fs::File::open(path).map_err(|error| fs_error("read file", path, &error))?;
    let mut buffer = [0u8; 8192];
    let read = file
        .read(&mut buffer)
        .map_err(|error| fs_error("read file", path, &error))?;
    let sample = &buffer[..read];
    if sample.contains(&0) {
        bail!("cannot read binary file: {}", path.display())
    }
    let non_printable = sample
        .iter()
        .filter(|byte| **byte < 9 || (**byte > 13 && **byte < 32))
        .count();
    if !sample.is_empty() && non_printable * 10 > sample.len() * 3 {
        bail!("cannot read binary file: {}", path.display())
    }
    Ok(())
}

/// 返回 macOS 截图文件名中普通空格与窄不换行空格互换后的备选路径。
///
/// 参数:
/// - `path`: 原始路径
///
/// 返回:
/// - 文件名以 ` AM.png` 或 ` PM.png` 结尾时的备选路径
pub(super) fn alternate_screenshot_path(path: &Path) -> Option<PathBuf> {
    let name = path.file_name()?.to_str()?;
    for (current, alternate) in [
        (" AM.png", "\u{202f}AM.png"),
        (" PM.png", "\u{202f}PM.png"),
        ("\u{202f}AM.png", " AM.png"),
        ("\u{202f}PM.png", " PM.png"),
    ] {
        if let Some(prefix) = name.strip_suffix(current) {
            if prefix.is_empty() {
                return None;
            }
            return Some(path.with_file_name(format!("{prefix}{alternate}")));
        }
    }
    None
}

/// 构造文件不存在时的错误，附带工作目录与相近路径建议。
///
/// 参数:
/// - `path`: 已展开路径
/// - `error`: 原始文件系统错误
///
/// 返回:
/// - 带定位建议的错误
pub(super) fn missing_file_error(path: &Path, error: &std::io::Error) -> anyhow::Error {
    let base = fs_error("read file", path, error);
    if error.kind() != std::io::ErrorKind::NotFound {
        return base;
    }
    let Ok(cwd) = crate::runtime_cwd::current_dir() else {
        return base;
    };
    let mut message = format!("{base}. {FILE_NOT_FOUND_CWD_NOTE} {}.", cwd.display());
    // 1. 路径误拼到工作区父目录时，优先建议工作区内的同名路径
    if let Some(suggestion) = suggest_path_under_cwd(path, &cwd) {
        message.push_str(&format!(" Did you mean {}?", suggestion.display()));
    } else if let Some(similar) = find_similar_file(path) {
        // 2. 其次建议同目录下主文件名相同、扩展名不同的文件
        message.push_str(&format!(" Did you mean {similar}?"));
    }
    anyhow::anyhow!(message)
}

/// 查找同目录下主文件名相同但扩展名不同的文件。
///
/// 参数:
/// - `path`: 不存在的目标路径
///
/// 返回:
/// - 相似文件名
fn find_similar_file(path: &Path) -> Option<String> {
    let directory = path.parent()?;
    let stem = path.file_stem()?;
    std::fs::read_dir(directory)
        .ok()?
        .filter_map(Result::ok)
        .find_map(|entry| {
            let candidate = entry.path();
            (candidate.file_stem() == Some(stem) && candidate != path)
                .then(|| entry.file_name().to_string_lossy().into_owned())
        })
}

/// 把误落在工作区父目录下的路径换算到工作区内。
///
/// 参数:
/// - `requested`: 请求路径
/// - `cwd`: 当前工作目录
///
/// 返回:
/// - 工作区内确实存在的对应路径
fn suggest_path_under_cwd(requested: &Path, cwd: &Path) -> Option<PathBuf> {
    let cwd_parent = cwd.parent()?;
    let resolved = requested
        .parent()
        .and_then(|parent| parent.canonicalize().ok())
        .and_then(|parent| requested.file_name().map(|name| parent.join(name)))
        .unwrap_or_else(|| requested.to_path_buf());
    if resolved == cwd_parent
        || !resolved.starts_with(cwd_parent)
        || resolved.starts_with(cwd)
        || resolved == cwd
    {
        return None;
    }
    let corrected = cwd.join(resolved.strip_prefix(cwd_parent).ok()?);
    std::fs::metadata(&corrected).ok().map(|_| corrected)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 设备黑名单覆盖标准输入输出与 /proc 文件描述符。
    #[test]
    fn blocks_devices_that_never_finish() {
        assert!(is_blocked_device_path(Path::new("/dev/zero")));
        assert!(is_blocked_device_path(Path::new("/proc/self/fd/0")));
        assert!(!is_blocked_device_path(Path::new("/dev/null")));
        assert!(!is_blocked_device_path(Path::new("/proc/cpuinfo")));
    }

    /// 二进制扩展名判定区分代码文件与压缩包。
    #[test]
    fn recognizes_binary_extensions() {
        assert!(has_binary_extension("zip"));
        assert!(has_binary_extension("pdf"));
        assert!(!has_binary_extension("rs"));
        assert!(!has_binary_extension(""));
    }

    /// macOS 截图文件名在两种空格之间互换。
    #[test]
    fn swaps_screenshot_space_variants() {
        let path = Path::new("/tmp/Screenshot 10.00.00 AM.png");
        let alternate = alternate_screenshot_path(path).unwrap();
        assert!(alternate.to_string_lossy().ends_with("\u{202f}AM.png"));
        assert!(alternate_screenshot_path(Path::new("/tmp/a.png")).is_none());
    }

    /// 同目录下不同扩展名的文件会作为建议给出。
    #[test]
    fn suggests_same_stem_with_other_extension() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("main.ts"), "x").unwrap();
        let error = std::io::Error::from(std::io::ErrorKind::NotFound);
        let message = missing_file_error(&temp.path().join("main.js"), &error).to_string();
        assert!(message.contains("does not exist"), "{message}");
        assert!(message.contains("Did you mean main.ts?"), "{message}");
    }

    /// 内容抽样能拦住含 NUL 的二进制数据。
    #[test]
    fn rejects_binary_content() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("blob");
        std::fs::write(&path, [0u8, 1, 2, 3]).unwrap();
        assert!(ensure_text_content(&path).is_err());
        std::fs::write(&path, "plain text\n").unwrap();
        assert!(ensure_text_content(&path).is_ok());
    }
}
