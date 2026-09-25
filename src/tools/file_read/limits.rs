//! read_file 读取上限。
//!
//! 数值与 CometixCode `file_read_tool/limits.rs`、`constants/api_limits.rs` 保持一致。

/// 未指定 limit 时整文件读取允许的最大字节数
pub(super) const MAX_SIZE_BYTES: u64 = 262_144;
/// 单次读取结果默认允许的最大 token 数
const DEFAULT_MAX_OUTPUT_TOKENS: usize = 25_000;
/// 覆盖单次读取 token 上限的环境变量
const MAX_OUTPUT_TOKENS_ENV: &str = "SAI_FILE_READ_MAX_OUTPUT_TOKENS";
/// 图片最大宽度，超出后等比缩放
pub(super) const IMAGE_MAX_WIDTH: u32 = 2_000;
/// 图片最大高度，超出后等比缩放
pub(super) const IMAGE_MAX_HEIGHT: u32 = 2_000;
/// 图片 base64 编码后的接口上限
pub(super) const API_IMAGE_MAX_BASE64_SIZE: usize = 5 * 1024 * 1024;
/// 图片原始字节目标上限，base64 膨胀后不超过接口上限
pub(super) const IMAGE_TARGET_RAW_SIZE: usize = API_IMAGE_MAX_BASE64_SIZE * 3 / 4;
/// 单次 PDF 分页读取允许的最大页数
pub(super) const PDF_MAX_PAGES_PER_READ: u32 = 20;
/// 未指定 pages 时允许整份读取的最大页数
pub(super) const PDF_INLINE_PAGE_THRESHOLD: u64 = 10;
/// 允许分页渲染的 PDF 最大字节数
pub(super) const PDF_MAX_EXTRACT_SIZE: u64 = 100 * 1024 * 1024;
/// 目录列表单页默认条数
pub(super) const DIRECTORY_PAGE_LIMIT: usize = 2_000;

/// 返回单次读取允许的最大 token 数。
///
/// 环境变量只接受正整数，其余取值回退到默认上限。
///
/// 返回:
/// - token 上限
pub(super) fn max_output_tokens() -> usize {
    std::env::var(MAX_OUTPUT_TOKENS_ENV)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_MAX_OUTPUT_TOKENS)
}

/// 把字节数格式化为 `12KB`、`3.4MB` 形式。
///
/// 参数:
/// - `bytes`: 字节数
///
/// 返回:
/// - 可读文件大小文本
pub(super) fn format_file_size(bytes: u64) -> String {
    let kb = bytes as f64 / 1024.0;
    if kb < 1.0 {
        return format!("{bytes} bytes");
    }
    if kb < 1024.0 {
        return format!("{}KB", one_decimal(kb));
    }
    let mb = kb / 1024.0;
    if mb < 1024.0 {
        return format!("{}MB", one_decimal(mb));
    }
    format!("{}GB", one_decimal(mb / 1024.0))
}

/// 保留一位小数并去掉多余的 `.0`。
///
/// 参数:
/// - `value`: 待格式化数值
///
/// 返回:
/// - 格式化文本
fn one_decimal(value: f64) -> String {
    let rendered = format!("{value:.1}");
    rendered
        .strip_suffix(".0")
        .map(str::to_string)
        .unwrap_or(rendered)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 文件大小格式与 CometixCode 的 formatFileSize 一致。
    #[test]
    fn formats_file_sizes_like_cometix() {
        assert_eq!(format_file_size(5), "5 bytes");
        assert_eq!(format_file_size(4096), "4KB");
        assert_eq!(format_file_size(1536), "1.5KB");
        assert_eq!(format_file_size(262_144), "256KB");
        assert_eq!(format_file_size(3 * 1024 * 1024), "3MB");
    }
}
