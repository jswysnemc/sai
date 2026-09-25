use super::limits::{format_file_size, max_output_tokens, MAX_SIZE_BYTES};
use crate::tools::fs_path::fs_error;
use anyhow::{bail, Result};
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

/// 按行范围读取的文本内容。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct TextRange {
    /// 选中行，已去掉行尾 `\r\n` / `\n`
    pub(super) lines: Vec<String>,
    /// 窗口为空时扫描得到的文件总行数；窗口非空时不保证完整
    pub(super) total_lines: usize,
    /// 文件字节数是否为 0
    pub(super) empty_file: bool,
}

/// 读取文本文件的指定行范围。
///
/// 未指定 limit 时整文件读取，超过 256KB 直接报错并提示使用 offset/limit；
/// 指定 limit 时流式读取窗口内的行，不受整文件大小限制。
///
/// 参数:
/// - `path`: 文件路径
/// - `start_index`: 0 起始的首行下标
/// - `limit`: 最多读取行数
///
/// 返回:
/// - 选中行与统计信息
pub(super) fn read_text_range(
    path: &Path,
    start_index: usize,
    limit: Option<usize>,
) -> Result<TextRange> {
    let metadata = std::fs::metadata(path).map_err(|error| fs_error("read file", path, &error))?;
    let empty_file = metadata.len() == 0;
    match limit {
        None => {
            // 1. 整文件读取先校验字节上限，避免把超大文件一次性载入
            if metadata.len() > MAX_SIZE_BYTES {
                bail!(
                    "File content ({}) exceeds maximum allowed size ({}). Use offset and limit parameters to read specific portions of the file, or search for specific content instead of reading the whole file.",
                    format_file_size(metadata.len()),
                    format_file_size(MAX_SIZE_BYTES)
                )
            }
            let mut bytes = Vec::with_capacity(metadata.len() as usize);
            std::fs::File::open(path)
                .and_then(|mut file| file.read_to_end(&mut bytes))
                .map_err(|error| fs_error("read file", path, &error))?;
            let all = split_lines(&String::from_utf8_lossy(&bytes));
            let total_lines = all.len();
            let lines = all.into_iter().skip(start_index).collect();
            Ok(TextRange {
                lines,
                total_lines,
                empty_file,
            })
        }
        Some(limit) => {
            // 2. 窗口读取逐行扫描，窗口填满即停止
            let file =
                std::fs::File::open(path).map_err(|error| fs_error("read file", path, &error))?;
            let mut reader = BufReader::new(file);
            let mut buffer = Vec::new();
            let mut lines = Vec::new();
            let mut index = 0usize;
            loop {
                buffer.clear();
                let read = reader
                    .read_until(b'\n', &mut buffer)
                    .map_err(|error| fs_error("read file", path, &error))?;
                if read == 0 {
                    break;
                }
                if index >= start_index {
                    if lines.len() >= limit {
                        break;
                    }
                    lines.push(decode_line(&buffer, index == 0));
                }
                index += 1;
            }
            Ok(TextRange {
                lines,
                total_lines: index,
                empty_file,
            })
        }
    }
}

/// 把整段文本拆成行，去掉 BOM 与行尾换行符。
///
/// 结尾换行之后的空片段不计为一行，与 `cat -n` 的行数一致。
///
/// 参数:
/// - `text`: 文件全文
///
/// 返回:
/// - 行列表
fn split_lines(text: &str) -> Vec<String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    if text.is_empty() {
        return Vec::new();
    }
    let body = text.strip_suffix('\n').unwrap_or(text);
    body.split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line).to_string())
        .collect()
}

/// 解码单行字节，去掉行尾换行与首行 BOM。
///
/// 参数:
/// - `bytes`: 含行尾换行的原始字节
/// - `first_line`: 是否为文件首行
///
/// 返回:
/// - 解码后的行文本
fn decode_line(bytes: &[u8], first_line: bool) -> String {
    let bytes = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    let bytes = bytes.strip_suffix(b"\r").unwrap_or(bytes);
    let text = String::from_utf8_lossy(bytes);
    if first_line {
        text.strip_prefix('\u{feff}').unwrap_or(&text).to_string()
    } else {
        text.into_owned()
    }
}

/// 按 `行号<TAB>正文` 格式给读取结果加行号。
///
/// 参数:
/// - `lines`: 选中行
/// - `first_line_number`: 首行的 1 起始行号
///
/// 返回:
/// - 带行号的文本
pub(super) fn add_line_numbers(lines: &[String], first_line_number: usize) -> String {
    lines
        .iter()
        .enumerate()
        .map(|(index, line)| format!("{}\t{line}", first_line_number + index))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 校验读取内容的 token 数没有超过单次上限。
///
/// 先用字节粗估，只有粗估超过上限四分之一时才做精确分词计数。
///
/// 参数:
/// - `content`: 读取到的正文
/// - `extension`: 小写扩展名，JSON 类文件按每 token 2 字节估算
///
/// 返回:
/// - 未超限时成功，否则返回提示使用 offset/limit 的错误
pub(super) fn validate_content_tokens(content: &str, extension: &str) -> Result<()> {
    let max_tokens = max_output_tokens();
    let bytes_per_token = match extension {
        "json" | "jsonl" | "jsonc" => 2,
        _ => 4,
    };
    let estimate = (content.encode_utf16().count() + bytes_per_token / 2) / bytes_per_token;
    if estimate == 0 || estimate <= max_tokens / 4 {
        return Ok(());
    }
    let tokens = crate::token_estimate::estimate_tokens(content);
    if tokens > max_tokens {
        bail!(
            "File content ({tokens} tokens) exceeds maximum allowed tokens ({max_tokens}). Use offset and limit parameters to read specific portions of the file, or search for specific content instead of reading the whole file."
        )
    }
    Ok(())
}

/// 生成文本读取的模型可见结果。
///
/// 参数:
/// - `range`: 行范围读取结果
/// - `first_line_number`: 请求的 1 起始行号
///
/// 返回:
/// - 带行号正文；空文件或 offset 越界时返回系统提醒
pub(super) fn render_text_result(range: &TextRange, first_line_number: usize) -> String {
    if !range.lines.is_empty() {
        return add_line_numbers(&range.lines, first_line_number);
    }
    if range.empty_file {
        return "<system-reminder>Warning: the file exists but the contents are empty.</system-reminder>"
            .to_string();
    }
    format!(
        "<system-reminder>Warning: the file exists but is shorter than the provided offset ({first_line_number}). The file has {} lines.</system-reminder>",
        range.total_lines
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 写入临时文件并返回路径。
    fn temp_file(content: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("sample.txt");
        std::fs::write(&path, content).unwrap();
        (temp, path)
    }

    /// 整文件读取去掉 BOM 与 CRLF，结尾换行不产生额外空行。
    #[test]
    fn whole_file_strips_bom_crlf_and_trailing_newline() {
        let (_temp, path) = temp_file(b"\xef\xbb\xbfa\r\nb\nc\n");
        let range = read_text_range(&path, 0, None).unwrap();
        assert_eq!(range.lines, vec!["a", "b", "c"]);
        assert_eq!(range.total_lines, 3);
        assert_eq!(render_text_result(&range, 1), "1\ta\n2\tb\n3\tc");
    }

    /// 窗口读取只返回 offset/limit 覆盖的行。
    #[test]
    fn window_read_returns_requested_lines() {
        let (_temp, path) = temp_file(b"one\ntwo\nthree\nfour\n");
        let range = read_text_range(&path, 1, Some(2)).unwrap();
        assert_eq!(range.lines, vec!["two", "three"]);
        assert_eq!(render_text_result(&range, 2), "2\ttwo\n3\tthree");
    }

    /// offset 越界时提示文件实际行数。
    #[test]
    fn offset_past_end_reports_total_lines() {
        let (_temp, path) = temp_file(b"one\ntwo\n");
        let range = read_text_range(&path, 9, Some(5)).unwrap();
        let rendered = render_text_result(&range, 10);
        assert!(
            rendered.contains("shorter than the provided offset (10)"),
            "{rendered}"
        );
        assert!(rendered.contains("The file has 2 lines"), "{rendered}");
    }

    /// 空文件给出空内容提醒。
    #[test]
    fn empty_file_reports_empty_contents() {
        let (_temp, path) = temp_file(b"");
        let range = read_text_range(&path, 0, None).unwrap();
        assert!(render_text_result(&range, 1).contains("contents are empty"));
    }

    /// 未指定 limit 时超过 256KB 的文件报错并提示分段读取。
    #[test]
    fn whole_file_over_size_limit_is_rejected() {
        let (_temp, path) = temp_file(&vec![b'a'; MAX_SIZE_BYTES as usize + 1]);
        let error = read_text_range(&path, 0, None).unwrap_err().to_string();
        assert!(
            error.contains("exceeds maximum allowed size (256KB)"),
            "{error}"
        );
        assert!(read_text_range(&path, 0, Some(1)).is_ok());
    }

    /// 超过 token 上限的内容被拒绝，普通内容通过。
    #[test]
    fn token_limit_rejects_oversized_content() {
        assert!(validate_content_tokens("fn main() {}", "rs").is_ok());
        let words = "alpha beta gamma delta ".repeat(40_000);
        let error = validate_content_tokens(&words, "txt")
            .unwrap_err()
            .to_string();
        assert!(error.contains("exceeds maximum allowed tokens"), "{error}");
    }
}
