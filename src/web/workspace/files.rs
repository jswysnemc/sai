use super::path_guard::{existing_path, mutable_existing_path, writable_path};
use anyhow::{bail, Context, Result};
use serde::Serialize;
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
    Ok(FileContent {
        path: relative.to_string(),
        content,
        size: metadata.len(),
        modified_at: metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs()),
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
pub(crate) fn write_file(root: &Path, relative: &str, content: &str) -> Result<FileContent> {
    if content.len() as u64 > MAX_FILE_BYTES {
        bail!("file exceeds {} bytes", MAX_FILE_BYTES);
    }
    let path = writable_path(root, relative)?;
    if path.exists() && !path.is_file() {
        bail!("path is not a regular file");
    }
    let parent = path.parent().context("file path has no parent")?;
    let temp = tempfile::NamedTempFile::new_in(parent)?;
    std::fs::write(temp.path(), content.as_bytes())?;
    temp.persist(&path)?;
    read_file(root, relative)
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
