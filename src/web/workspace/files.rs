use super::path_guard::{existing_path, mutable_existing_path, writable_path};
use super::file_write_lock::with_file_write_lock;
use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::fmt;
use std::fs::OpenOptions;
use std::path::Path;
use std::time::UNIX_EPOCH;

const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_IMAGE_BYTES: u64 = 20 * 1024 * 1024;
const MAX_TREE_DEPTH: usize = 8;
const IGNORED_DIRECTORIES: &[&str] = &[".git", "node_modules", "target", "dist", "build"];

/// 文件树节点。
#[derive(Clone, Debug, Serialize)]
pub(crate) struct FileNode {
    pub name: String,
    pub path: String,
    pub kind: &'static str,
    pub children: Vec<FileNode>,
}

/// 文本文件内容。
#[derive(Clone, Debug, Serialize)]
pub(crate) struct FileContent {
    pub path: String,
    pub content: String,
    pub size: u64,
    pub modified_at: Option<u64>,
    /// 文件内容指纹，用于避免秒级修改时间无法识别的并发覆盖
    pub version: String,
}

/// 工作区文件变更结果。
#[derive(Clone, Debug, Serialize)]
pub(crate) struct FileMutation {
    pub path: String,
    pub kind: &'static str,
}

/// 可在编辑器中预览的图像文件。
pub(crate) struct ImageFile {
    pub mime: String,
    pub bytes: Vec<u8>,
}

/// 工作区文件内容版本不匹配。
#[derive(Debug)]
pub(crate) struct FileVersionConflict;

impl fmt::Display for FileVersionConflict {
    /// 返回供 API 映射冲突响应使用的错误文本。
    ///
    /// 参数:
    /// - `formatter`: 错误文本格式化器
    ///
    /// 返回:
    /// - 格式化结果
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("file changed outside the editor")
    }
}

impl std::error::Error for FileVersionConflict {}

/// 读取工作区文件树。
///
/// 参数:
/// - `root`: 工作区根目录
/// - `relative`: 起始相对目录
/// - `depth`: 最大递归深度
///
/// 返回:
/// - 文件树节点
pub(crate) fn read_tree(root: &Path, relative: &str, depth: usize) -> Result<Vec<FileNode>> {
    let path = existing_path(root, relative)?;
    if !path.is_dir() {
        bail!("tree path is not a directory");
    }
    read_directory(root, &path, depth.clamp(1, MAX_TREE_DEPTH))
}

/// 读取 UTF-8 文本文件。
pub(crate) fn read_file(root: &Path, relative: &str) -> Result<FileContent> {
    let path = existing_path(root, relative)?;
    let metadata = std::fs::metadata(&path)?;
    if !metadata.is_file() {
        bail!("path is not a file");
    }
    if metadata.len() > MAX_FILE_BYTES {
        bail!("file exceeds {} bytes", MAX_FILE_BYTES);
    }
    let bytes = std::fs::read(&path)?;
    if bytes.contains(&0) {
        bail!("binary files are not supported");
    }
    let content = String::from_utf8(bytes).context("file is not valid UTF-8")?;
    let version = blake3::hash(content.as_bytes()).to_hex().to_string();
    Ok(FileContent {
        path: relative.to_string(),
        content,
        size: metadata.len(),
        modified_at: metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs()),
        version,
    })
}

/// 读取工作区图像文件。
///
/// 参数:
/// - `root`: 工作区根目录
/// - `relative`: 图像相对路径
///
/// 返回:
/// - 图像 MIME 与原始字节
pub(crate) fn read_image(root: &Path, relative: &str) -> Result<ImageFile> {
    let path = existing_path(root, relative)?;
    let metadata = std::fs::metadata(&path)?;
    if !metadata.is_file() {
        bail!("path is not a file");
    }
    if metadata.len() > MAX_IMAGE_BYTES {
        bail!("image exceeds {} bytes", MAX_IMAGE_BYTES);
    }
    let mime = mime_guess::from_path(&path)
        .first()
        .filter(|mime| mime.type_() == mime_guess::mime::IMAGE)
        .context("file is not a supported image")?;
    Ok(ImageFile {
        mime: mime.to_string(),
        bytes: std::fs::read(path)?,
    })
}

#[cfg(test)]
mod image_tests {
    use super::*;

    /// 验证编辑器图像接口读取 PNG 字节和 MIME。
    #[test]
    fn reads_image_file_for_preview() {
        let temp = tempfile::tempdir().unwrap();
        let bytes = [0x89, b'P', b'N', b'G'];
        std::fs::write(temp.path().join("preview.png"), bytes).unwrap();

        let image = read_image(temp.path(), "preview.png").unwrap();

        assert_eq!(image.mime, "image/png");
        assert_eq!(image.bytes, bytes);
    }
}

/// 原子保存 UTF-8 文本文件。
///
/// 参数:
/// - `root`: 工作区根目录
/// - `relative`: 文件相对路径
/// - `content`: 待保存文本
/// - `expected_version`: 编辑器读取时的内容指纹
///
/// 返回:
/// - 保存后的文件内容；版本不匹配时返回 `FileVersionConflict`
pub(crate) fn write_file(
    root: &Path,
    relative: &str,
    content: &str,
    expected_version: Option<&str>,
) -> Result<FileContent> {
    if content.len() as u64 > MAX_FILE_BYTES {
        bail!("file exceeds {} bytes", MAX_FILE_BYTES);
    }
    let path = writable_path(root, relative)?;
    with_file_write_lock(&path, || {
        write_file_locked(root, relative, content, expected_version, &path)
    })
}

/// 在路径锁内校验版本并原子替换文件。
///
/// 参数:
/// - `root`: 工作区根目录
/// - `relative`: 文件相对路径
/// - `content`: 待保存文本
/// - `expected_version`: 编辑器读取时的内容指纹
/// - `path`: 已通过路径保护的写入路径
///
/// 返回:
/// - 保存后的文件内容；版本不匹配时返回 `FileVersionConflict`
fn write_file_locked(
    root: &Path,
    relative: &str,
    content: &str,
    expected_version: Option<&str>,
    path: &Path,
) -> Result<FileContent> {
    if path.exists() && !path.is_file() {
        bail!("path is not a regular file");
    }
    validate_expected_version(root, relative, expected_version)?;
    // 1. 覆盖已有文件时复制原权限，避免临时文件的 0600 覆盖执行位或组权限
    let original_permissions = if path.is_file() {
        Some(std::fs::metadata(&path)?.permissions())
    } else {
        None
    };
    let parent = path.parent().context("file path has no parent")?;
    let temp = tempfile::NamedTempFile::new_in(parent)?;
    std::fs::write(temp.path(), content.as_bytes())?;
    if let Some(permissions) = original_permissions {
        std::fs::set_permissions(temp.path(), permissions)?;
    }
    // 2. 临时文件准备完成后紧邻替换操作再次校验，覆盖保存期间发生的外部修改
    validate_expected_version(root, relative, expected_version)?;
    temp.persist(&path)?;
    read_file(root, relative)
}

/// 校验工作区文件当前内容指纹。
///
/// 参数:
/// - `root`: 工作区根目录
/// - `relative`: 文件相对路径
/// - `expected_version`: 可选预期内容指纹
///
/// 返回:
/// - 指纹一致或未要求校验时返回成功
fn validate_expected_version(
    root: &Path,
    relative: &str,
    expected_version: Option<&str>,
) -> Result<()> {
    let Some(expected_version) = expected_version else {
        return Ok(());
    };
    let current = read_file(root, relative).map_err(|_| FileVersionConflict)?;
    if current.version != expected_version {
        return Err(FileVersionConflict.into());
    }
    Ok(())
}

#[cfg(test)]
mod write_tests {
    use super::*;

    #[test]
    fn content_version_changes_when_content_changes() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("file.txt"), "first").unwrap();
        let first = read_file(temp.path(), "file.txt").unwrap();

        std::fs::write(temp.path().join("file.txt"), "second").unwrap();
        let second = read_file(temp.path(), "file.txt").unwrap();

        assert_ne!(first.version, second.version);
    }

    #[cfg(unix)]
    #[test]
    fn write_file_preserves_existing_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("script.sh");
        std::fs::write(&path, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o750)).unwrap();

        write_file(temp.path(), "script.sh", "#!/bin/sh\necho ok\n", None).unwrap();

        let mode = std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o750);
    }

    #[test]
    fn write_file_rejects_stale_content_version() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("file.txt"), "first").unwrap();
        let original = read_file(temp.path(), "file.txt").unwrap();
        std::fs::write(temp.path().join("file.txt"), "external").unwrap();

        let error =
            write_file(temp.path(), "file.txt", "editor", Some(&original.version)).unwrap_err();

        assert!(error.downcast_ref::<FileVersionConflict>().is_some());
        assert_eq!(
            std::fs::read_to_string(temp.path().join("file.txt")).unwrap(),
            "external"
        );
    }

    #[test]
    fn concurrent_writes_with_one_version_allow_only_one_replace() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("file.txt"), "first").unwrap();
        let original = read_file(temp.path(), "file.txt").unwrap();
        let root = std::sync::Arc::new(temp.path().to_path_buf());
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let mut handles = Vec::new();
        for content in ["second", "third"] {
            let root = root.clone();
            let barrier = barrier.clone();
            let version = original.version.clone();
            handles.push(std::thread::spawn(move || {
                barrier.wait();
                write_file(&root, "file.txt", content, Some(&version))
            }));
        }

        let results = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| {
                    result
                        .as_ref()
                        .err()
                        .and_then(|error| error.downcast_ref::<FileVersionConflict>())
                        .is_some()
                })
                .count(),
            1
        );
    }
}

/// 创建空文件或目录。
///
/// 参数:
/// - `root`: 工作区根目录
/// - `relative`: 新条目相对路径
/// - `directory`: 是否创建目录
///
/// 返回:
/// - 创建后的条目摘要
pub(crate) fn create_entry(root: &Path, relative: &str, directory: bool) -> Result<FileMutation> {
    let path = writable_path(root, relative)?;
    if path.exists() {
        bail!("path already exists");
    }
    if directory {
        std::fs::create_dir(&path)?;
    } else {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
    }
    Ok(FileMutation {
        path: relative.to_string(),
        kind: if directory { "directory" } else { "file" },
    })
}

/// 重命名工作区文件或目录。
///
/// 参数:
/// - `root`: 工作区根目录
/// - `from`: 原相对路径
/// - `to`: 新相对路径
///
/// 返回:
/// - 重命名后的条目摘要
pub(crate) fn rename_entry(root: &Path, from: &str, to: &str) -> Result<FileMutation> {
    let source = mutable_existing_path(root, from)?;
    let target = writable_path(root, to)?;
    if target.exists() {
        bail!("target path already exists");
    }
    let metadata = std::fs::symlink_metadata(&source)?;
    std::fs::rename(source, target)?;
    Ok(FileMutation {
        path: to.to_string(),
        kind: if metadata.is_dir() {
            "directory"
        } else {
            "file"
        },
    })
}

/// 删除工作区文件或目录。
///
/// 参数:
/// - `root`: 工作区根目录
/// - `relative`: 待删除相对路径
///
/// 返回:
/// - 删除前的条目摘要
pub(crate) fn delete_entry(root: &Path, relative: &str) -> Result<FileMutation> {
    let path = mutable_existing_path(root, relative)?;
    let metadata = std::fs::symlink_metadata(&path)?;
    let directory = metadata.is_dir() && !metadata.file_type().is_symlink();
    if directory {
        std::fs::remove_dir_all(path)?;
    } else {
        std::fs::remove_file(path)?;
    }
    Ok(FileMutation {
        path: relative.to_string(),
        kind: if directory { "directory" } else { "file" },
    })
}

/// 递归读取单个目录。
fn read_directory(root: &Path, directory: &Path, depth: usize) -> Result<Vec<FileNode>> {
    let mut entries = std::fs::read_dir(directory)?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| {
        let is_file = entry.file_type().map(|kind| kind.is_file()).unwrap_or(true);
        (
            is_file,
            entry.file_name().to_string_lossy().to_ascii_lowercase(),
        )
    });
    let mut nodes = Vec::new();
    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        if IGNORED_DIRECTORIES.contains(&name.as_str()) {
            continue;
        }
        let path = entry.path();
        let file_type = entry.file_type()?;
        let kind = if file_type.is_dir() {
            "directory"
        } else if file_type.is_symlink() {
            "symlink"
        } else {
            "file"
        };
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        let children = if file_type.is_dir() && depth > 1 {
            read_directory(root, &path, depth - 1)?
        } else {
            Vec::new()
        };
        nodes.push(FileNode {
            name,
            path: relative,
            kind,
            children,
        });
    }
    Ok(nodes)
}
