//! HTML 预览授权：把工作区里的 HTML 文件交给内置浏览器。
//!
//! 内置浏览器只允许 http/https。这里签发一次性路径，页面和它引用的样式、脚本、图片
//! 都从同一授权根目录读取。工作区内的文件以工作区为根；工作区外的文件只开放它所在的目录。

use super::path_guard::editor_existing_path;
use anyhow::{bail, Context, Result};
use base64::Engine;
use rand::RngCore;
use std::collections::{HashMap, VecDeque};
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, OnceLock};

const MAX_PREVIEW_BYTES: u64 = 32 * 1024 * 1024;
const MAX_GRANTS: usize = 32;

struct PreviewGrant {
    root: PathBuf,
}

struct PreviewGrants {
    by_token: HashMap<String, PreviewGrant>,
    by_root: HashMap<PathBuf, String>,
    order: VecDeque<String>,
}

/**
 * 【工作区】【HTML 预览】取出进程内的预览授权表。
 * @returns 授权表锁
 */
fn grants() -> std::sync::MutexGuard<'static, PreviewGrants> {
    static GRANTS: OnceLock<Mutex<PreviewGrants>> = OnceLock::new();
    GRANTS
        .get_or_init(|| {
            Mutex::new(PreviewGrants {
                by_token: HashMap::new(),
                by_root: HashMap::new(),
                order: VecDeque::new(),
            })
        })
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

/**
 * 【工作区】【HTML 预览】为 HTML 文件签发内置浏览器可打开的路径。
 * @param workspace_root 为当前工作区根目录
 * @param requested 为工作区相对路径或已存在的绝对路径
 * @returns 以 `/api/html-preview/` 开头的路径，不含主机名
 */
pub(crate) fn html_preview_url(workspace_root: &Path, requested: &str) -> Result<String> {
    let file = editor_existing_path(workspace_root, requested)?;
    if !file.is_file() {
        bail!("path is not a file");
    }
    if !is_html_file(&file) {
        bail!("only HTML files can be opened in the built-in browser");
    }
    let workspace = workspace_root
        .canonicalize()
        .context("workspace root does not exist")?;
    // 1. 工作区内允许引用工作区里的相对资源；工作区外只开放该文件所在目录
    let root = if file.starts_with(&workspace) {
        workspace
    } else {
        file.parent()
            .context("html file has no parent directory")?
            .to_path_buf()
    };
    let relative = file
        .strip_prefix(&root)
        .context("html file is outside its preview root")?
        .to_string_lossy()
        .replace('\\', "/");
    let token = grant_token(root);
    Ok(format!(
        "/api/html-preview/{token}/{}",
        encode_relative(&relative)
    ))
}

/**
 * 【工作区】【HTML 预览】按授权读取一个预览文件。
 * @param token 为预览授权
 * @param relative 为授权根目录下的相对路径
 * @returns 文件字节与媒体类型
 */
pub(crate) fn read_html_preview(token: &str, relative: &str) -> Result<(Vec<u8>, &'static str)> {
    let root = grant_root(token)?;
    let relative = safe_relative(relative)?;
    let path = root.join(&relative);
    let canonical = path
        .canonicalize()
        .with_context(|| format!("path does not exist: {}", path.display()))?;
    if !canonical.starts_with(&root) {
        bail!("path escapes preview root");
    }
    let metadata = std::fs::metadata(&canonical)?;
    if !metadata.is_file() {
        bail!("path is not a file");
    }
    if metadata.len() > MAX_PREVIEW_BYTES {
        bail!("file exceeds {MAX_PREVIEW_BYTES} bytes");
    }
    let bytes = std::fs::read(&canonical)?;
    Ok((bytes, preview_mime(&canonical)))
}

/**
 * 【工作区】【HTML 预览】判断文件扩展名是不是 HTML。
 * @param path 为文件路径
 * @returns `.html` 或 `.htm` 时为 true
 */
fn is_html_file(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("html") || value.eq_ignore_ascii_case("htm"))
}

/**
 * 【工作区】【HTML 预览】复用同一目录的授权，超出上限时丢掉最早的一个。
 * @param root 为已经规范化的授权根目录
 * @returns 预览令牌
 */
fn grant_token(root: PathBuf) -> String {
    let mut grants = grants();
    if let Some(token) = grants.by_root.get(&root) {
        return token.clone();
    }
    let token = new_token();
    while grants.order.len() >= MAX_GRANTS {
        if let Some(old) = grants.order.pop_front() {
            if let Some(grant) = grants.by_token.remove(&old) {
                grants.by_root.remove(&grant.root);
            }
        }
    }
    grants.by_root.insert(root.clone(), token.clone());
    grants.order.push_back(token.clone());
    grants.by_token.insert(token.clone(), PreviewGrant { root });
    token
}

/**
 * 【工作区】【HTML 预览】取出授权根目录。
 * @param token 为预览令牌
 * @returns 规范化后的根目录
 */
fn grant_root(token: &str) -> Result<PathBuf> {
    if !is_token(token) {
        bail!("preview is unavailable");
    }
    let grants = grants();
    grants
        .by_token
        .get(token)
        .map(|grant| grant.root.clone())
        .context("preview is unavailable")
}

/**
 * 【工作区】【HTML 预览】生成不可猜测的预览令牌。
 * @returns URL 安全的令牌
 */
fn new_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/**
 * 【工作区】【HTML 预览】校验令牌字符，避免把它当成路径。
 * @param token 为路径中的令牌
 * @returns 字符合法时为 true
 */
fn is_token(token: &str) -> bool {
    (16..=128).contains(&token.len())
        && token
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-' || character == '_')
}

/**
 * 【工作区】【HTML 预览】把相对路径编成 URL 片段。
 * @param relative 为使用 `/` 的相对路径
 * @returns 逐段编码后的路径
 */
fn encode_relative(relative: &str) -> String {
    relative
        .split('/')
        .map(|segment| urlencoding::encode(segment).into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/**
 * 【工作区】【HTML 预览】拒绝绝对路径和上级目录。
 * @param relative 为浏览器请求的路径
 * @returns 可拼到授权根上的相对路径
 */
fn safe_relative(relative: &str) -> Result<PathBuf> {
    let relative = relative.trim().trim_start_matches('/');
    if relative.is_empty() {
        bail!("path is required");
    }
    let path = PathBuf::from(relative);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        bail!("path escapes preview root");
    }
    Ok(path)
}

/**
 * 【工作区】【HTML 预览】按扩展名选择媒体类型。
 * @param path 为预览文件
 * @returns 带字符集的文本类型，或对应的二进制类型
 */
fn preview_mime(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "wasm" => "application/wasm",
        "txt" | "xml" | "csv" => "text/plain; charset=utf-8",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_html_can_read_sibling_assets_but_not_parent_paths() {
        let workspace = tempfile::tempdir().unwrap();
        let docs = workspace.path().join("docs");
        std::fs::create_dir(&docs).unwrap();
        std::fs::write(docs.join("index.html"), "<link rel=\"stylesheet\" href=\"app.css\">").unwrap();
        std::fs::write(docs.join("app.css"), "body{}").unwrap();

        let url = html_preview_url(workspace.path(), "docs/index.html").unwrap();
        let token = url
            .trim_start_matches("/api/html-preview/")
            .split('/')
            .next()
            .unwrap();
        let page = read_html_preview(token, "docs/index.html").unwrap();
        let style = read_html_preview(token, "docs/app.css").unwrap();

        assert!(url.contains("/docs/index.html"));
        assert!(page.0.starts_with(b"<link"));
        assert_eq!(page.1, "text/html; charset=utf-8");
        assert_eq!(style.1, "text/css; charset=utf-8");
        assert!(read_html_preview(token, "docs/../docs/index.html").is_err());
        assert!(read_html_preview(token, "/etc/passwd").is_err());
    }

    #[test]
    fn external_html_cannot_read_outside_its_directory() {
        let workspace = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let site = outside.path().join("site");
        std::fs::create_dir(&site).unwrap();
        let page = site.join("page.html");
        std::fs::write(&page, "<h1>outside</h1>").unwrap();
        std::fs::write(site.join("app.js"), "console.log(1)").unwrap();
        std::fs::write(outside.path().join("secret.txt"), "secret").unwrap();

        let url = html_preview_url(workspace.path(), page.to_str().unwrap()).unwrap();
        let token = url
            .trim_start_matches("/api/html-preview/")
            .split('/')
            .next()
            .unwrap();

        assert!(read_html_preview(token, "page.html").is_ok());
        assert!(read_html_preview(token, "app.js").is_ok());
        assert!(read_html_preview(token, "../secret.txt").is_err());
    }

    #[test]
    fn rejects_non_html_and_unknown_tokens() {
        let workspace = tempfile::tempdir().unwrap();
        std::fs::write(workspace.path().join("notes.md"), "# notes").unwrap();

        assert!(html_preview_url(workspace.path(), "notes.md").is_err());
        assert!(read_html_preview("not-a-real-token", "index.html").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_outside_the_grant_root_is_rejected() {
        let workspace = tempfile::tempdir().unwrap();
        std::fs::write(workspace.path().join("index.html"), "<p>ok</p>").unwrap();
        std::os::unix::fs::symlink("/etc/passwd", workspace.path().join("leak.txt")).unwrap();
        let url = html_preview_url(workspace.path(), "index.html").unwrap();
        let token = url
            .trim_start_matches("/api/html-preview/")
            .split('/')
            .next()
            .unwrap();

        assert!(read_html_preview(token, "leak.txt").is_err());
    }
}
