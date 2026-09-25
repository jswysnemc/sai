//! read_file 工具。
//!
//! 读取语义参考 CometixCode 的 Read：文本按 `行号<TAB>正文` 返回，整文件读取受
//! 256KB 与 token 上限约束；图片经缩放压缩后直接交给当前多模态模型；
//! 另支持 Jupyter notebook 与 PDF 分页。目录分页是 sai 额外保留的能力。

use super::{ToolOutput, ToolRegistry, ToolSpec};
use crate::i18n::text as t;
use anyhow::{bail, Result};
use serde_json::{json, Value};

mod directory;
mod guard;
mod image;
mod image_resize;
mod limits;
mod notebook;
mod pdf;
mod request;
mod text;

use request::ReadRequest;

/// 注册 read_file 工具。
///
/// 参数:
/// - `registry`: 工具注册表
///
/// 返回:
/// - 无
pub fn register(registry: &mut ToolRegistry) {
    registry.register(ToolSpec::new_with_output(
        "read_file",
        t(
            "Read a file from the local filesystem. Text is returned with line numbers (line number, tab, content); images are shown directly to you; PDF pages are rendered as images; Jupyter notebooks return every cell with outputs; a directory returns a listing. Without offset/limit the whole file is read (text files over 256KB must be read in ranges).",
            "读取本地文件。文本按“行号、制表符、正文”返回；图片直接交给你查看；PDF 页面渲染为图片；Jupyter notebook 返回全部单元格及输出；目录返回列表。未指定 offset/limit 时读取整个文件（超过 256KB 的文本需分段读取）。",
        ),
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": t("Absolute path, or a path relative to the workspace, of the file to read.", "要读取的文件的绝对路径或相对工作区路径。")
                },
                "offset": {
                    "type": "integer",
                    "description": t("The line number to start reading from (1-based). Only provide if the file is too large to read at once.", "开始读取的行号（从 1 开始）。仅在文件过大无法一次读完时提供。")
                },
                "limit": {
                    "type": "integer",
                    "description": t("The number of lines to read. Only provide if the file is too large to read at once.", "要读取的行数。仅在文件过大无法一次读完时提供。")
                },
                "pages": {
                    "type": "string",
                    "description": t("Page range for PDF files (e.g., \"1-5\", \"3\", \"10-20\"). Only applicable to PDF files. Maximum 20 pages per request.", "PDF 页码范围（如 \"1-5\"、\"3\"、\"10-20\"）。仅适用于 PDF，每次最多 20 页。")
                }
            },
            "required": ["path"],
            "additionalProperties": false
        }),
        |args| async move { read_file(args).await },
    ));
}

/// 按文件类型分派读取。
///
/// 1. 校验 pages、设备文件与二进制扩展名
/// 2. 目录返回列表，缺失文件尝试截图备选路径并给出建议
/// 3. notebook、图片、PDF 与文本分别走各自的读取器
///
/// 参数:
/// - `args`: 工具参数
///
/// 返回:
/// - 模型可见文本与可选图片附件
async fn read_file(args: Value) -> Result<ToolOutput> {
    let mut request = ReadRequest::from_value(&args)?;
    if let Some(pages) = request.pages.as_deref() {
        pdf::validate_pages(pages)?;
    }
    if guard::is_blocked_device_path(&request.path) {
        bail!(
            "Cannot read '{}': this device file would block or produce infinite output.",
            request.raw_path
        )
    }
    let extension = request.extension();
    if guard::has_binary_extension(&extension)
        && extension != "pdf"
        && !image::is_image_extension(&extension)
    {
        bail!(
            "This tool cannot read binary files. The file appears to be a binary .{extension} file. Please use appropriate tools for binary file analysis."
        )
    }
    // 2. 路径不存在时先试 macOS 截图文件名的空格变体，再给出定位建议
    let metadata = match std::fs::metadata(&request.path) {
        Ok(metadata) => metadata,
        Err(error) => match guard::alternate_screenshot_path(&request.path).and_then(|alternate| {
            std::fs::metadata(&alternate)
                .ok()
                .map(|meta| (alternate, meta))
        }) {
            Some((alternate, metadata)) => {
                request.path = alternate;
                metadata
            }
            None => return Err(guard::missing_file_error(&request.path, &error)),
        },
    };
    if metadata.is_dir() {
        return directory::read_directory(&request.path, request.offset, request.limit)
            .map(ToolOutput::text);
    }
    if !metadata.is_file() {
        bail!(
            "not a regular file or directory: {}",
            request.path.display()
        )
    }
    // 3. 按扩展名分派
    match extension.as_str() {
        "ipynb" => notebook::read_notebook(&request.path),
        "pdf" => pdf::read_pdf(&request.path, request.pages.as_deref()).await,
        ext if image::is_image_extension(ext) => image::read_image(&request.path),
        _ => read_text(&request, &extension),
    }
}

/// 读取文本文件并加上行号。
///
/// 参数:
/// - `request`: 读取请求
/// - `extension`: 小写扩展名
///
/// 返回:
/// - 带行号的正文或空文件、越界提醒
fn read_text(request: &ReadRequest, extension: &str) -> Result<ToolOutput> {
    guard::ensure_text_content(&request.path)?;
    let range = text::read_text_range(&request.path, request.offset - 1, request.limit)?;
    text::validate_content_tokens(&range.lines.join("\n"), extension)?;
    Ok(ToolOutput::text(text::render_text_result(
        &range,
        request.offset,
    )))
}

#[cfg(test)]
mod tests;
