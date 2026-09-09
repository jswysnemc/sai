use anyhow::{bail, Context, Result};
use std::ffi::OsString;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use windows_sys::Win32::Storage::FileSystem::GetShortPathNameW;

const MAX_CURRENT_DIRECTORY: usize = 260;
const MAX_WIDE_PATH: u32 = 32768;

/// 【插件进程】【Windows 工作目录】为子进程选择指向原目录且符合 CreateProcess 长度限制的路径。
/// @param path 已通过宿主目录授权的绝对路径
/// @returns 可直接传入子进程的路径；无法取得可用短路径时明确失败
pub(super) fn working_directory(path: &Path) -> Result<PathBuf> {
    // 【插件进程】【Windows 工作目录】1. 普通短路径不需要访问文件系统或更改表示
    let compatible = dunce::simplified(path);
    if fits_current_directory(compatible) {
        return Ok(compatible.to_path_buf());
    }
    let canonical = std::fs::canonicalize(path).context("resolve plugin process directory")?;
    let short = short_path(&canonical)?;
    let compatible = dunce::simplified(&short);
    if !fits_current_directory(compatible) {
        bail!("plugin process directory exceeds the Windows current-directory limit and has no usable short path");
    }
    // 【插件进程】【Windows 工作目录】2. 系统别名必须仍指向授权目录，不能重新确定目录归属
    if std::fs::canonicalize(compatible)? != canonical {
        bail!("plugin process short path no longer matches the authorized directory");
    }
    Ok(compatible.to_path_buf())
}

/// 【插件进程】【Windows 长度】按 UTF-16 计算目录、终止符及系统补充的尾部分隔符。
/// @param path 将交给 CreateProcess 的目录表示
/// @returns 完整工作目录是否能放入系统缓冲区
fn fits_current_directory(path: &Path) -> bool {
    let mut length = 1usize;
    let mut last = None;
    for unit in path.as_os_str().encode_wide() {
        length += 1;
        last = Some(unit);
        if length > MAX_CURRENT_DIRECTORY {
            return false;
        }
    }
    length + usize::from(!matches!(last, Some(47 | 92))) <= MAX_CURRENT_DIRECTORY
}

/// 【插件进程】【Windows 短路径】查询系统已有的目录别名，限制宽字符缓冲区并拒绝截断结果。
/// @param path 已解析的完整规范目录
/// @returns 操作系统返回的短路径；不修改文件名、全局工作目录或卷设置
fn short_path(path: &Path) -> Result<PathBuf> {
    let mut wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .take(MAX_WIDE_PATH as usize)
        .collect();
    if wide.len() >= MAX_WIDE_PATH as usize || wide.contains(&0) {
        bail!("plugin process directory exceeds the supported Windows path format");
    }
    wide.push(0);
    // 【插件进程】【Windows 短路径】1. 先取得所需大小，再使用有界缓冲区读取完整结果
    let required = unsafe { GetShortPathNameW(wide.as_ptr(), std::ptr::null_mut(), 0) };
    if required == 0 {
        return Err(std::io::Error::last_os_error()).context("query plugin process short path");
    }
    if required > MAX_WIDE_PATH {
        bail!("plugin process short path exceeds the supported Windows path size");
    }
    let mut buffer = vec![0u16; required as usize];
    let written = unsafe { GetShortPathNameW(wide.as_ptr(), buffer.as_mut_ptr(), required) };
    if written == 0 {
        return Err(std::io::Error::last_os_error()).context("read plugin process short path");
    }
    if written >= required {
        bail!("plugin process short path changed while resolving the directory");
    }
    Ok(PathBuf::from(OsString::from_wide(
        &buffer[..written as usize],
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【插件进程测试】【目录边界】终止符、尾部反斜杠及代理对都计入 Windows 的字符上限。
    #[test]
    fn windows_directory_limit_reserves_terminator_and_separator() {
        let maximum = format!("C:\\{}", "a".repeat(255));
        assert!(fits_current_directory(Path::new(&maximum)));
        assert!(fits_current_directory(Path::new(&format!("{maximum}\\"))));
        assert!(!fits_current_directory(Path::new(&format!("{maximum}b"))));
        assert!(fits_current_directory(Path::new(&format!(
            "C:\\{}",
            "\u{10400}".repeat(127)
        ))));
        assert!(!fits_current_directory(Path::new(&format!(
            "C:\\{}",
            "\u{10400}".repeat(128)
        ))));
    }
}
