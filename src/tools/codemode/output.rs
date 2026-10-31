use crate::tools::ToolModelAttachment;
use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use sai_codemode::{output::Output, protocol::OutputKind};
use serde::Deserialize;
use std::path::Path;

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScriptOptions {
    max_output_tokens: Option<usize>,
    timeout_ms: Option<u64>,
}

/// 【CodeMode】【脚本选项】解析 code 的首行选项，按 timeout 上限返回超时和输出预算。
pub(crate) fn options(code: &str, timeout: u64) -> Result<(u64, usize)> {
    let mut options = ScriptOptions::default();
    for (index, line) in code.lines().enumerate() {
        if let Some(json) = line.trim().strip_prefix("// @options:") {
            if index != 0 {
                bail!("codemode options must be on the first line");
            }
            options = serde_json::from_str(json).context("invalid codemode options")?;
        }
    }
    let timeout = options.timeout_ms.unwrap_or(timeout).min(timeout);
    let budget = options.max_output_tokens.unwrap_or(10_000);
    if timeout == 0 || budget == 0 || budget > 100_000 {
        bail!("invalid codemode timeout or output budget");
    }
    Ok((timeout, budget))
}

/// 【CodeMode】【输出渲染】把 output/error 转为预算内文本及图片，directory 保存完整附件。
pub(crate) fn render(
    output: Output,
    error: Option<&str>,
    directory: &Path,
    budget: usize,
) -> Result<(String, Vec<ToolModelAttachment>, Option<String>)> {
    let mut failure = error.map(str::to_owned);
    let mut text = String::new();
    let mut console = Vec::new();
    let mut attachments = Vec::new();
    for (kind, value) in output.items {
        match kind {
            OutputKind::Text => {
                text.push_str(&value);
                text.push('\n');
            }
            OutputKind::Console => console.push(value),
            OutputKind::Image => match image_attachment(value, directory) {
                Ok(attachment) => {
                    text.push_str(&attachment.note);
                    text.push('\n');
                    attachments.push(attachment);
                }
                Err(image_error) => {
                    failure.get_or_insert_with(|| image_error.to_string());
                }
            },
        }
    }
    if !console.is_empty() {
        text.push_str("<console_output>\n");
        text.push_str(&console.join("\n"));
        text.push_str("\n</console_output>\n");
    }
    if let Some(error) = &failure {
        text.push_str("\nScript error: ");
        text.push_str(error);
    }
    let text = truncate(&text, directory, budget.saturating_mul(4))?;
    let text = match &failure {
        Some(_) => format!("tool error: Script failed\n{text}"),
        None => format!("Script completed\n{text}"),
    };
    Ok((text, attachments, failure))
}

/// 【CodeMode】【图片适配】验证 value 并保存到 directory，返回模型附件；失败不影响先前文本。
fn image_attachment(value: String, directory: &Path) -> Result<ToolModelAttachment> {
    let (prefix, data) = value.split_once(",").context("invalid codemode image")?;
    let extension = match prefix {
        "data:image/png;base64" => "png",
        "data:image/jpeg;base64" => "jpg",
        "data:image/gif;base64" => "gif",
        "data:image/webp;base64" => "webp",
        _ => bail!("unsupported codemode image"),
    };
    let data = STANDARD.decode(data.replace(['\r', '\n'], ""))?;
    // 1. 【CodeMode】【图片校验】确认实际图片可解码，限制解码分配，避免伪造图片进入模型
    let mut reader = image::ImageReader::new(std::io::Cursor::new(&data)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(64 * 1024 * 1024);
    limits.max_image_width = Some(16_384);
    limits.max_image_height = Some(16_384);
    reader.limits(limits);
    reader.decode().context("invalid codemode image data")?;
    std::fs::create_dir_all(directory)?;
    let path = directory.join(format!(
        "codemode-image-{}.{}",
        uuid::Uuid::new_v4(),
        extension
    ));
    std::fs::write(&path, &data)?;
    let note = format!("Script image: {}", path.display());
    Ok(ToolModelAttachment::new(
        value,
        path.display().to_string(),
        note,
    ))
}

/// 【CodeMode】【文本截断】按 max_chars 保留 text 首尾并落盘，返回包含文件引用的文本。
fn truncate(text: &str, directory: &Path, max_chars: usize) -> Result<String> {
    if text.chars().count() <= max_chars {
        return Ok(text.into());
    }
    std::fs::create_dir_all(directory)?;
    let path = directory.join(format!("codemode-output-{}.txt", uuid::Uuid::new_v4()));
    std::fs::write(&path, text)?;
    let head: String = text.chars().take(max_chars / 2).collect();
    let tail: String = text
        .chars()
        .rev()
        .take(max_chars / 2)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    Ok(format!(
        "{head}\n... output truncated; full output: {} ...\n{tail}",
        path.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_image_keeps_prior_text_and_marks_script_failed() {
        let directory = tempfile::tempdir().unwrap();
        let mut output = Output::default();
        output
            .push(OutputKind::Text, "partial text".into())
            .unwrap();
        output
            .push(OutputKind::Image, "data:image/png;base64,aA==".into())
            .unwrap();
        let (text, attachments, error) = render(output, None, directory.path(), 100).unwrap();
        assert!(text.starts_with("tool error:"));
        assert!(text.contains("partial text"));
        assert!(error.is_some());
        assert!(attachments.is_empty());
    }

    #[test]
    fn script_errors_share_output_budget_and_are_archived() {
        let directory = tempfile::tempdir().unwrap();
        let error = "error".repeat(1000);
        let (text, _, _) = render(Output::default(), Some(&error), directory.path(), 10).unwrap();
        assert!(text.contains("output truncated"));
        assert!(text.len() < error.len());
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn options_cannot_extend_host_deadline_and_must_be_first_line() {
        assert_eq!(
            options(
                "// @options: {\"timeout_ms\":9000,\"max_output_tokens\":10}\nreturn 1",
                1000
            )
            .unwrap(),
            (1000, 10)
        );
        assert!(options("text(1)\n// @options: {}", 1000).is_err());
        assert!(options("// @options: {\"timeout_ms\":0}", 1000).is_err());
    }

    #[test]
    fn truncation_keeps_unicode_head_tail_and_full_output_file() {
        let dir = tempfile::tempdir().unwrap();
        let full = "开头".to_owned() + &"正文".repeat(100) + "结尾";
        let rendered = truncate(&full, dir.path(), 20).unwrap();
        assert!(rendered.starts_with("开头"));
        assert!(rendered.ends_with("结尾"));
        let path = std::fs::read_dir(dir.path())
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(std::fs::read_to_string(path).unwrap(), full);
    }
}
