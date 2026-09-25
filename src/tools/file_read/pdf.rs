//! PDF 分页读取，对应 CometixCode `utils/pdf.rs` 与 `utils/pdf_utils.rs`。
//!
//! 页面由 poppler-utils 的 `pdftoppm` 渲染为 JPEG，再作为图片附件交给当前模型。

use super::image_resize::{image_metadata_text, process_image};
use super::limits::{
    format_file_size, PDF_INLINE_PAGE_THRESHOLD, PDF_MAX_EXTRACT_SIZE, PDF_MAX_PAGES_PER_READ,
};
use crate::tools::fs_path::fs_error;
use crate::tools::{ToolModelAttachment, ToolOutput};
use anyhow::{anyhow, bail, Result};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::Command;

/// pdfinfo 超时
const PDFINFO_TIMEOUT: Duration = Duration::from_secs(10);
/// pdftoppm 渲染超时
const PDFTOPPM_TIMEOUT: Duration = Duration::from_secs(120);

/// 1 起始的页码范围；`last` 为空表示到文档末尾。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PageRange {
    pub(super) first: u32,
    pub(super) last: Option<u32>,
}

/// 解析 `3`、`1-5`、`10-` 形式的页码范围。
///
/// 参数:
/// - `pages`: 页码范围原文
///
/// 返回:
/// - 合法范围；格式错误或页码小于 1 时为 None
pub(super) fn parse_page_range(pages: &str) -> Option<PageRange> {
    let trimmed = pages.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(first) = trimmed.strip_suffix('-') {
        let first = first
            .trim()
            .parse::<u32>()
            .ok()
            .filter(|value| *value >= 1)?;
        return Some(PageRange { first, last: None });
    }
    if let Some((first, last)) = trimmed.split_once('-') {
        let first = first
            .trim()
            .parse::<u32>()
            .ok()
            .filter(|value| *value >= 1)?;
        let last = last
            .trim()
            .parse::<u32>()
            .ok()
            .filter(|value| *value >= first)?;
        return Some(PageRange {
            first,
            last: Some(last),
        });
    }
    let page = trimmed.parse::<u32>().ok().filter(|value| *value >= 1)?;
    Some(PageRange {
        first: page,
        last: Some(page),
    })
}

/// 校验 pages 参数的格式与页数上限。
///
/// 参数:
/// - `pages`: 页码范围原文
///
/// 返回:
/// - 合法页码范围
pub(super) fn validate_pages(pages: &str) -> Result<PageRange> {
    let Some(range) = parse_page_range(pages) else {
        bail!(
            "Invalid pages parameter: \"{pages}\". Use formats like \"1-5\", \"3\", or \"10-20\". Pages are 1-indexed."
        )
    };
    let size = range
        .last
        .map(|last| last - range.first + 1)
        .unwrap_or(PDF_MAX_PAGES_PER_READ + 1);
    if size > PDF_MAX_PAGES_PER_READ {
        bail!(
            "Page range \"{pages}\" exceeds maximum of {PDF_MAX_PAGES_PER_READ} pages per request. Please use a smaller range."
        )
    }
    Ok(range)
}

/// 读取 PDF：指定 pages 时渲染这些页，否则在页数不多时渲染整份文档。
///
/// 参数:
/// - `path`: PDF 路径
/// - `pages`: 可选页码范围
///
/// 返回:
/// - 文本说明与每页图片附件
pub(super) async fn read_pdf(path: &Path, pages: Option<&str>) -> Result<ToolOutput> {
    let metadata = std::fs::metadata(path).map_err(|error| fs_error("read PDF", path, &error))?;
    let size = metadata.len();
    if size == 0 {
        bail!("PDF file is empty: {}", path.display())
    }
    if size > PDF_MAX_EXTRACT_SIZE {
        bail!(
            "PDF file exceeds maximum allowed size for page extraction ({}).",
            format_file_size(PDF_MAX_EXTRACT_SIZE)
        )
    }
    ensure_pdf_header(path)?;
    // 1. 未指定 pages 时按页数决定是否允许整份读取
    let range = match pages {
        Some(pages) => validate_pages(pages)?,
        None => match page_count(path).await {
            Some(count) if count > PDF_INLINE_PAGE_THRESHOLD => bail!(
                "This PDF has {count} pages, which is too many to read at once. Use the pages parameter to read specific page ranges (e.g., pages: \"1-5\"). Maximum {PDF_MAX_PAGES_PER_READ} pages per request."
            ),
            Some(count) => PageRange {
                first: 1,
                last: Some(count.max(1) as u32),
            },
            None => PageRange {
                first: 1,
                last: Some(PDF_INLINE_PAGE_THRESHOLD as u32),
            },
        },
    };
    // 2. 渲染页面并逐页转换为模型附件
    let workdir = tempfile::tempdir()?;
    let page_files = render_pages(path, range, workdir.path()).await?;
    let mut attachments = Vec::with_capacity(page_files.len());
    for (index, file) in page_files.iter().enumerate() {
        let bytes = std::fs::read(file)?;
        let processed = process_image(&bytes, usize::MAX)?;
        let source = format!("{} page {}", path.display(), range.first as usize + index);
        let note = image_metadata_text(&source, &processed, bytes.len() as u64);
        attachments.push(ToolModelAttachment::new(processed.data_url(), source, note));
    }
    let content = format!(
        "PDF pages extracted: {} page(s) from {} ({})",
        attachments.len(),
        path.display(),
        format_file_size(size)
    );
    Ok(ToolOutput::text(content).with_model_attachments(attachments))
}

/// 校验文件头是否为 PDF。
fn ensure_pdf_header(path: &Path) -> Result<()> {
    use std::io::Read;
    let mut header = [0u8; 5];
    let read = std::fs::File::open(path)
        .and_then(|mut file| file.read(&mut header))
        .map_err(|error| fs_error("read PDF", path, &error))?;
    if &header[..read] != b"%PDF-" {
        bail!(
            "File is not a valid PDF (missing %PDF- header): {}",
            path.display()
        )
    }
    Ok(())
}

/// 用 pdfinfo 读取页数；工具缺失或解析失败时返回 None。
async fn page_count(path: &Path) -> Option<u64> {
    let output = tokio::time::timeout(
        PDFINFO_TIMEOUT,
        Command::new("pdfinfo")
            .arg(path)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .ok()?
    .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("Pages:"))
        .and_then(|value| value.trim().parse().ok())
}

/// 用 pdftoppm 把指定页渲染为 100 DPI 的 JPEG。
///
/// 参数:
/// - `path`: PDF 路径
/// - `range`: 页码范围
/// - `output_dir`: 输出目录
///
/// 返回:
/// - 按页码排序的图片文件
async fn render_pages(path: &Path, range: PageRange, output_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut command = Command::new("pdftoppm");
    command.args(["-jpeg", "-r", "100", "-f", &range.first.to_string()]);
    if let Some(last) = range.last {
        command.args(["-l", &last.to_string()]);
    }
    command
        .arg(path)
        .arg(output_dir.join("page"))
        .kill_on_drop(true);
    let output = match tokio::time::timeout(PDFTOPPM_TIMEOUT, command.output()).await {
        Err(_) => bail!("pdftoppm timed out after {}s", PDFTOPPM_TIMEOUT.as_secs()),
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => bail!(
            "pdftoppm is not installed. Install poppler-utils (e.g. `brew install poppler`, `apt-get install poppler-utils` or `pacman -S poppler`) to enable PDF page rendering."
        ),
        Ok(Err(error)) => return Err(anyhow!("failed to run pdftoppm: {error}")),
        Ok(Ok(output)) => output,
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.to_ascii_lowercase().contains("password") {
            bail!("PDF is password-protected. Please provide an unprotected version.")
        }
        bail!("pdftoppm failed: {}", stderr.trim())
    }
    let mut files = std::fs::read_dir(output_dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("jpg"))
        .collect::<Vec<_>>();
    files.sort();
    if files.is_empty() {
        bail!(
            "pdftoppm produced no pages for range {}{}",
            range.first,
            range
                .last
                .map(|last| format!("-{last}"))
                .unwrap_or_default()
        )
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 页码范围解析覆盖单页、闭区间与开区间。
    #[test]
    fn parses_page_ranges() {
        assert_eq!(
            parse_page_range("3"),
            Some(PageRange {
                first: 3,
                last: Some(3)
            })
        );
        assert_eq!(
            parse_page_range(" 1-5 "),
            Some(PageRange {
                first: 1,
                last: Some(5)
            })
        );
        assert_eq!(
            parse_page_range("10-"),
            Some(PageRange {
                first: 10,
                last: None
            })
        );
        assert_eq!(parse_page_range("0"), None);
        assert_eq!(parse_page_range("5-2"), None);
        assert_eq!(parse_page_range("abc"), None);
    }

    /// 超过 20 页或开区间范围被拒绝。
    #[test]
    fn rejects_ranges_over_the_page_limit() {
        assert!(validate_pages("1-20").is_ok());
        assert!(validate_pages("1-21")
            .unwrap_err()
            .to_string()
            .contains("exceeds maximum of 20 pages"));
        assert!(validate_pages("3-").is_err());
        assert!(validate_pages("x")
            .unwrap_err()
            .to_string()
            .contains("Invalid pages parameter"));
    }

    /// 非 PDF 文件头直接报错。
    #[tokio::test]
    async fn rejects_files_without_pdf_header() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("fake.pdf");
        std::fs::write(&path, "not a pdf").unwrap();
        let error = read_pdf(&path, Some("1")).await.unwrap_err().to_string();
        assert!(error.contains("missing %PDF- header"), "{error}");
    }
}
