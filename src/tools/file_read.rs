use super::{ToolModelAttachment, ToolOutput, ToolRegistry, ToolSpec};
use crate::config::AppConfig;
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

mod image;

const MAX_READ_BYTES: usize = 50 * 1024;
const MAX_BATCH_BYTES: usize = 100 * 1024;
const MAX_BATCH_FILES: usize = 10;
const MAX_READ_LINES: usize = 2_000;
const MAX_LINE_CHARS: usize = 2_000;

pub fn register(registry: &mut ToolRegistry, config: AppConfig, paths: SaiPaths) {
    registry.register(ToolSpec::new_with_output(
        "read_file",
        t(
            "Read one or more UTF-8 text files by 1-based line offset, list directory pages, or analyze local images with the vision model. Use path for one target or files for batch reads.",
            "按 1 起始行号读取一个或多个 UTF-8 文本文件、分页列出目录，或使用视觉模型分析本地图片。单个目标使用 path，批量读取使用 files。",
        ),
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": t("Single file or directory path.", "单个文件或目录路径。")
                },
                "offset": {
                    "type": "integer",
                    "description": t("Starting line or directory entry, 1-based. Defaults to 1.", "起始行或目录项，1 起始。默认 1。")
                },
                "limit": {
                    "type": "integer",
                    "description": t("Maximum lines or directory entries. Defaults to 2000.", "最多读取行数或目录项数量。默认 2000。")
                },
                "image_prompt": {
                    "type": "string",
                    "description": t("Optional prompt used when path points to a local image.", "当 path 指向本地图片时使用的可选读图提示。")
                },
                "files": {
                    "type": "array",
                    "description": t("Batch file or directory pages. Each item supports path, offset, and limit.", "批量文件或目录分页。每一项支持 path、offset、limit。"),
                    "items": {
                        "type": "object",
                        "properties": {
                            "path": {"type": "string", "description": t("File or directory path.", "文件或目录路径。")},
                            "offset": {"type": "integer", "description": t("Starting line or directory entry, 1-based.", "起始行或目录项，1 起始。")},
                            "limit": {"type": "integer", "description": t("Maximum lines or directory entries.", "最多读取行数或目录项数量。")},
                            "image_prompt": {"type": "string", "description": t("Optional prompt used when path points to a local image.", "当 path 指向本地图片时使用的可选读图提示。")}
                        },
                        "required": ["path"],
                        "additionalProperties": false
                    },
                    "maxItems": MAX_BATCH_FILES
                }
            },
            "additionalProperties": false
        }),
        move |args| {
            let config = config.clone();
            let paths = paths.clone();
            async move { read_file(args, config, paths).await }
        },
    ));
}

/// 读取单个或批量文件内容。
///
/// 参数:
/// - `args`: 工具参数，支持 path 或 files
/// - `config`: 应用配置
/// - `paths`: Sai 路径
///
/// 返回:
/// - JSON 格式读取结果
async fn read_file(args: Value, config: AppConfig, paths: SaiPaths) -> Result<ToolOutput> {
    let accept_model_attachments = args
        .get("_sai_model_attachments")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if let Some(files) = args.get("files") {
        return read_files(files, &config, &paths, accept_model_attachments).await;
    }
    let request = ReadRequest::from_value(&args, accept_model_attachments)?;
    let page = read_page(&request, MAX_READ_BYTES, &config, &paths).await?;
    Ok(ToolOutput::text(serde_json::to_string_pretty(&page.value)?)
        .with_model_attachments(page.model_attachments))
}

/// 批量读取多个文件或目录分页。
///
/// 参数:
/// - `files`: 批量读取项
/// - `config`: 应用配置
/// - `paths`: Sai 路径
///
/// 返回:
/// - JSON 格式批量读取结果
async fn read_files(
    files: &Value,
    config: &AppConfig,
    paths: &SaiPaths,
    accept_model_attachments: bool,
) -> Result<ToolOutput> {
    let Some(items) = files.as_array() else {
        bail!("files must be an array")
    };
    if items.is_empty() {
        bail!("files must not be empty")
    }
    if items.len() > MAX_BATCH_FILES {
        bail!("files contains too many items: max {MAX_BATCH_FILES}")
    }
    let mut used_bytes = 0usize;
    let mut results = Vec::new();
    let mut model_attachments = Vec::new();
    for item in items {
        if used_bytes >= MAX_BATCH_BYTES {
            results.push(json!({
                "ok": false,
                "type": "error",
                "path": item.get("path").and_then(Value::as_str).unwrap_or_default(),
                "error": "batch output byte limit reached before reading this item",
            }));
            continue;
        }
        let remaining = MAX_BATCH_BYTES
            .saturating_sub(used_bytes)
            .min(MAX_READ_BYTES);
        let result = match ReadRequest::from_value(item, accept_model_attachments) {
            Ok(request) => match read_page(&request, remaining, config, paths).await {
                Ok(page) => {
                    used_bytes += page.value.to_string().len();
                    model_attachments.extend(page.model_attachments);
                    page.value
                }
                Err(err) => json!({
                    "ok": false,
                    "type": "error",
                    "path": item.get("path").and_then(Value::as_str).unwrap_or_default(),
                    "error": err.to_string(),
                }),
            },
            Err(err) => json!({
                "ok": false,
                "type": "error",
                "path": item.get("path").and_then(Value::as_str).unwrap_or_default(),
                "error": err.to_string(),
            }),
        };
        results.push(result);
    }
    Ok(ToolOutput::text(serde_json::to_string_pretty(&json!({
        "type": "multi-text-page",
        "count": results.len(),
        "max_batch_bytes": MAX_BATCH_BYTES,
        "results": results,
    }))?)
    .with_model_attachments(model_attachments))
}

pub(super) struct ReadRequest {
    pub(super) path: PathBuf,
    pub(super) offset: usize,
    pub(super) limit: usize,
    pub(super) image_prompt: Option<String>,
    pub(super) accept_model_attachment: bool,
}

/// 单个读取页面及其下一次模型请求附件。
pub(super) struct ReadPage {
    pub(super) value: Value,
    pub(super) model_attachments: Vec<ToolModelAttachment>,
}

impl ReadPage {
    /// 创建不包含模型附件的普通读取页面。
    ///
    /// 参数:
    /// - `value`: 工具可见 JSON 值
    ///
    /// 返回:
    /// - 普通读取页面
    pub(super) fn text(value: Value) -> Self {
        Self {
            value,
            model_attachments: Vec::new(),
        }
    }
}

impl ReadRequest {
    /// 从 JSON 参数解析读取请求。
    ///
    /// 参数:
    /// - `args`: 单个读取请求参数
    ///
    /// 返回:
    /// - 读取请求
    fn from_value(args: &Value, accept_model_attachment: bool) -> Result<Self> {
        Ok(Self {
            path: path_arg(args, "path")?,
            offset: args
                .get("offset")
                .and_then(Value::as_u64)
                .unwrap_or(1)
                .max(1) as usize,
            limit: args
                .get("limit")
                .and_then(Value::as_u64)
                .unwrap_or(MAX_READ_LINES as u64)
                .clamp(1, MAX_READ_LINES as u64) as usize,
            image_prompt: args
                .get("image_prompt")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string),
            accept_model_attachment,
        })
    }
}

/// 读取一个路径的分页内容。
///
/// 参数:
/// - `request`: 读取请求
/// - `byte_budget`: 本次读取最大字节预算
/// - `config`: 应用配置
/// - `paths`: Sai 路径
///
/// 返回:
/// - JSON 值形式的分页内容
async fn read_page(
    request: &ReadRequest,
    byte_budget: usize,
    config: &AppConfig,
    paths: &SaiPaths,
) -> Result<ReadPage> {
    if request.path.is_dir() {
        return read_directory_page(request).map(ReadPage::text);
    }
    let metadata = std::fs::metadata(&request.path)?;
    if !metadata.is_file() {
        bail!(
            "not a regular file or directory: {}",
            request.path.display()
        )
    }
    if is_image_file(&request.path) {
        return image::read_image_page(request, config, paths).await;
    }
    ensure_not_binary_file(&request.path)?;
    read_text_page(request, byte_budget).map(ReadPage::text)
}

/// 判断路径是否为常见图片文件。
///
/// 参数:
/// - `path`: 文件路径
///
/// 返回:
/// - 是否为图片文件
fn is_image_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp")
    )
}

/// 读取目录分页。
///
/// 参数:
/// - `request`: 读取请求
///
/// 返回:
/// - 目录分页 JSON
fn read_directory_page(request: &ReadRequest) -> Result<Value> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(&request.path)? {
        let entry = entry?;
        let suffix = if entry.file_type()?.is_dir() { "/" } else { "" };
        entries.push(format!("{}{}", entry.file_name().to_string_lossy(), suffix));
    }
    entries.sort();
    let start = request.offset.saturating_sub(1);
    let selected = entries
        .iter()
        .skip(start)
        .take(request.limit)
        .cloned()
        .collect::<Vec<_>>();
    let next = (start + selected.len() < entries.len()).then_some(request.offset + selected.len());
    Ok(json!({
        "type": "directory-page",
        "path": request.path.display().to_string(),
        "offset": request.offset,
        "limit": request.limit,
        "entries": selected,
        "truncated": next.is_some(),
        "next": next,
    }))
}

/// 读取文本文件分页。
///
/// 参数:
/// - `request`: 读取请求
/// - `byte_budget`: 本次读取最大字节预算
///
/// 返回:
/// - 文本分页 JSON
fn read_text_page(request: &ReadRequest, byte_budget: usize) -> Result<Value> {
    let file = std::fs::File::open(&request.path)?;
    let reader = BufReader::new(file);
    let mut lines = Vec::new();
    let mut bytes = 0usize;
    let mut next = None;
    for (index, line) in reader.lines().enumerate() {
        let line_number = index + 1;
        if line_number < request.offset {
            continue;
        }
        if lines.len() >= request.limit || bytes >= byte_budget {
            next = Some(line_number);
            break;
        }
        let mut line = line?;
        if line.chars().count() > MAX_LINE_CHARS {
            line = format!(
                "{}... (line truncated to {MAX_LINE_CHARS} chars)",
                line.chars().take(MAX_LINE_CHARS).collect::<String>()
            );
        }
        let rendered = format!("{line_number}: {line}");
        bytes += rendered.len() + 1;
        if bytes > byte_budget {
            next = Some(line_number);
            break;
        }
        lines.push(rendered);
    }
    if lines.is_empty() && request.offset != 1 {
        bail!("offset {} is out of range", request.offset)
    }
    Ok(json!({
        "type": "text-page",
        "path": request.path.display().to_string(),
        "offset": request.offset,
        "limit": request.limit,
        "content": lines.join("\n"),
        "truncated": next.is_some(),
        "next": next,
    }))
}

/// 检查文件是否看起来是二进制文件。
///
/// 参数:
/// - `path`: 文件路径
///
/// 返回:
/// - 文件是否可作为文本读取
fn ensure_not_binary_file(path: &Path) -> Result<()> {
    let mut file = std::fs::File::open(path)?;
    let mut buffer = [0u8; 8192];
    let read = file.read(&mut buffer)?;
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

/// 读取必填路径参数。
///
/// 参数:
/// - `args`: JSON 参数
/// - `key`: 字段名
///
/// 返回:
/// - 展开后的路径
fn path_arg(args: &Value, key: &str) -> Result<PathBuf> {
    let value = args
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if value.is_empty() {
        bail!("{}: {key}", t("required argument missing", "缺少必需参数"))
    }
    Ok(expand_path(value))
}

/// 展开路径文本。
///
/// 参数:
/// - `value`: 路径文本
///
/// 返回:
/// - 绝对或当前目录相对路径
fn expand_path(value: &str) -> PathBuf {
    let value = value.trim();
    if let Some(rest) = value.strip_prefix("~/") {
        if let Some(home) = directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf()) {
            return home.join(rest);
        }
    }
    let path = Path::new(value);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        crate::runtime_cwd::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::paths::SaiPaths;

    fn test_paths(root: &Path) -> SaiPaths {
        SaiPaths {
            config_dir: root.join("config"),
            config_file: root.join("config/config.jsonc"),
            secrets_file: root.join("config/secrets.jsonc"),
            skills_dir: root.join("config/skills"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            state_dir: root.join("state"),
            pictures_dir: root.join("pictures"),
            fish_hook_file: root.join("fish/sai.fish"),
            bash_hook_file: root.join("shell/bash-hook.sh"),
            zsh_hook_file: root.join("shell/zsh-hook.zsh"),
            powershell_hook_file: root.join("shell/powershell-hook.ps1"),
        }
    }

    #[tokio::test]
    async fn read_file_paginates_text() {
        let cwd = crate::runtime_cwd::current_dir().unwrap();
        let temp = tempfile::tempdir_in(cwd).unwrap();
        let paths = test_paths(temp.path());
        let path = temp.path().join("sample.txt");
        std::fs::write(&path, "one\ntwo\nthree\n").unwrap();
        let result = read_file(
            json!({
                "path": path.display().to_string(),
                "offset": 2,
                "limit": 1,
            }),
            AppConfig::default(),
            paths,
        )
        .await
        .unwrap();
        let data: Value = serde_json::from_str(&result.content).unwrap();
        assert_eq!(data["type"], "text-page");
        assert_eq!(data["content"], "2: two");
        assert_eq!(data["truncated"], true);
        assert_eq!(data["next"], 3);
    }

    #[tokio::test]
    async fn read_file_reads_multiple_files() {
        let cwd = crate::runtime_cwd::current_dir().unwrap();
        let temp = tempfile::tempdir_in(cwd).unwrap();
        let paths = test_paths(temp.path());
        let first = temp.path().join("first.txt");
        let second = temp.path().join("second.txt");
        std::fs::write(&first, "a1\na2\n").unwrap();
        std::fs::write(&second, "b1\nb2\n").unwrap();
        let result = read_file(
            json!({
                "files": [
                    {"path": first.display().to_string(), "offset": 2, "limit": 1},
                    {"path": second.display().to_string(), "limit": 1}
                ]
            }),
            AppConfig::default(),
            paths,
        )
        .await
        .unwrap();
        let data: Value = serde_json::from_str(&result.content).unwrap();
        assert_eq!(data["type"], "multi-text-page");
        assert_eq!(data["count"], 2);
        assert_eq!(data["results"][0]["content"], "2: a2");
        assert_eq!(data["results"][1]["content"], "1: b1");
        assert_eq!(data["results"][1]["next"], 2);
    }

    #[tokio::test]
    async fn read_file_batch_keeps_item_errors_local() {
        let cwd = crate::runtime_cwd::current_dir().unwrap();
        let temp = tempfile::tempdir_in(cwd).unwrap();
        let paths = test_paths(temp.path());
        let text = temp.path().join("sample.txt");
        let bin = temp.path().join("sample.bin");
        std::fs::write(&text, "ok\n").unwrap();
        std::fs::write(&bin, [0, 1, 2, 3]).unwrap();
        let result = read_file(
            json!({
                "files": [
                    {"path": text.display().to_string()},
                    {"path": bin.display().to_string()}
                ]
            }),
            AppConfig::default(),
            paths,
        )
        .await
        .unwrap();
        let data: Value = serde_json::from_str(&result.content).unwrap();
        assert_eq!(data["results"][0]["type"], "text-page");
        assert_eq!(data["results"][1]["ok"], false);
        assert!(data["results"][1]["error"]
            .as_str()
            .unwrap()
            .contains("cannot read binary file"));
    }

    #[tokio::test]
    async fn read_file_rejects_binary() {
        let cwd = crate::runtime_cwd::current_dir().unwrap();
        let temp = tempfile::tempdir_in(cwd).unwrap();
        let paths = test_paths(temp.path());
        let path = temp.path().join("sample.bin");
        std::fs::write(&path, [0, 1, 2, 3]).unwrap();
        assert!(read_file(
            json!({"path": path.display().to_string()}),
            AppConfig::default(),
            paths
        )
        .await
        .is_err());
    }
}
