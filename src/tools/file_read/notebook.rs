//! Jupyter notebook 读取，对应 CometixCode `utils/notebook.rs`。

use super::image_resize::{image_metadata_text, process_image};
use super::limits::{format_file_size, max_output_tokens, MAX_SIZE_BYTES};
use crate::tools::fs_path::fs_error;
use crate::tools::{ToolModelAttachment, ToolOutput};
use anyhow::{bail, Context, Result};
use base64::Engine;
use serde_json::Value;
use std::path::Path;

/// 单个单元格输出合计超过该字符数时只给出查看命令
const LARGE_OUTPUT_THRESHOLD: usize = 10_000;
/// 单条输出文本保留的最大字符数
const OUTPUT_TEXT_MAX_CHARS: usize = 30_000;

/// 解析后的单元格。
struct NotebookCell {
    id: String,
    cell_type: String,
    language: Option<String>,
    source: String,
    outputs: Vec<CellOutput>,
}

/// 单元格输出。
struct CellOutput {
    text: String,
    image: Option<(String, String)>,
}

/// 读取 notebook，返回单元格文本与输出图片附件。
///
/// 参数:
/// - `path`: notebook 路径
///
/// 返回:
/// - 文本为按单元格拼接的源码与输出，附件为输出中的图片
pub(super) fn read_notebook(path: &Path) -> Result<ToolOutput> {
    let bytes = std::fs::read(path).map_err(|error| fs_error("read notebook", path, &error))?;
    let notebook: Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("invalid notebook JSON: {}", path.display()))?;
    let cells = parse_cells(&notebook)?;
    // 1. 体积校验沿用整文件上限，超限时给出 jq 分段查看方式
    let size = serde_json::to_string(&notebook.get("cells"))
        .map(|text| text.len() as u64)
        .unwrap_or(bytes.len() as u64);
    if size > MAX_SIZE_BYTES {
        let shown = path.display();
        bail!(
            "Notebook content ({}) exceeds maximum allowed size ({}). Use run_command with jq to read specific portions:\n  cat \"{shown}\" | jq '.cells[:20]' # First 20 cells\n  cat \"{shown}\" | jq '.cells[100:120]' # Cells 100-120\n  cat \"{shown}\" | jq '.cells | length' # Count total cells\n  cat \"{shown}\" | jq '.cells[] | select(.cell_type==\"code\") | .source' # All code sources",
            format_file_size(size),
            format_file_size(MAX_SIZE_BYTES)
        )
    }
    // 2. 拼出单元格文本并收集输出图片
    let mut blocks = Vec::new();
    let mut attachments = Vec::new();
    for (index, cell) in cells.iter().enumerate() {
        blocks.push(render_cell(cell));
        for output in &cell.outputs {
            if !output.text.is_empty() {
                blocks.push(format!("\n{}", output.text));
            }
            if let Some((media_type, data)) = &output.image {
                if let Some(attachment) = output_image(path, index, media_type, data) {
                    blocks.push(format!("\n{}", attachment.note));
                    attachments.push(attachment);
                }
            }
        }
    }
    let content = blocks.join("\n");
    super::text::validate_content_tokens(&content, "ipynb")?;
    Ok(ToolOutput::text(content).with_model_attachments(attachments))
}

/// 解析 notebook 的全部单元格。
///
/// 参数:
/// - `notebook`: notebook JSON
///
/// 返回:
/// - 单元格列表
fn parse_cells(notebook: &Value) -> Result<Vec<NotebookCell>> {
    let language = notebook
        .pointer("/metadata/language_info/name")
        .and_then(Value::as_str)
        .unwrap_or("python")
        .to_string();
    let cells = notebook
        .get("cells")
        .and_then(Value::as_array)
        .context("notebook has no cells array")?;
    Ok(cells
        .iter()
        .enumerate()
        .map(|(index, cell)| parse_cell(cell, index, &language))
        .collect())
}

/// 解析单个单元格；超大输出替换为查看命令。
///
/// 参数:
/// - `cell`: 单元格 JSON
/// - `index`: 单元格下标
/// - `language`: notebook 代码语言
///
/// 返回:
/// - 单元格
fn parse_cell(cell: &Value, index: usize, language: &str) -> NotebookCell {
    let cell_type = cell
        .get("cell_type")
        .and_then(Value::as_str)
        .unwrap_or("code")
        .to_string();
    let is_code = cell_type == "code";
    let id = cell
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("cell-{index}"));
    let mut outputs = if is_code {
        cell.get("outputs")
            .and_then(Value::as_array)
            .map(|items| items.iter().filter_map(parse_output).collect::<Vec<_>>())
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let output_size = outputs
        .iter()
        .map(|output| {
            output.text.len()
                + output
                    .image
                    .as_ref()
                    .map(|(_, data)| data.len())
                    .unwrap_or(0)
        })
        .sum::<usize>();
    if output_size > LARGE_OUTPUT_THRESHOLD {
        outputs = vec![CellOutput {
            text: format!(
                "Outputs are too large to include. Use run_command with: cat <notebook_path> | jq '.cells[{index}].outputs'"
            ),
            image: None,
        }];
    }
    NotebookCell {
        id,
        language: is_code.then(|| language.to_string()),
        source: joined_text(cell.get("source")),
        cell_type,
        outputs,
    }
}

/// 解析单条输出，未知类型返回 None。
///
/// 参数:
/// - `output`: 输出 JSON
///
/// 返回:
/// - 输出文本与可选图片
fn parse_output(output: &Value) -> Option<CellOutput> {
    let kind = output.get("output_type")?.as_str()?;
    match kind {
        "stream" => Some(CellOutput {
            text: clip(&joined_text(output.get("text"))),
            image: None,
        }),
        "execute_result" | "display_data" => {
            let data = output.get("data");
            let text = clip(&joined_text(data.and_then(|data| data.get("text/plain"))));
            let image = data.and_then(|data| {
                ["image/png", "image/jpeg"].iter().find_map(|media_type| {
                    data.get(*media_type).map(|value| {
                        let encoded = joined_text(Some(value))
                            .chars()
                            .filter(|ch| !ch.is_whitespace())
                            .collect::<String>();
                        (media_type.to_string(), encoded)
                    })
                })
            });
            Some(CellOutput { text, image })
        }
        "error" => {
            let traceback = output
                .get("traceback")
                .and_then(Value::as_array)
                .map(|lines| {
                    lines
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();
            let name = output
                .get("ename")
                .and_then(Value::as_str)
                .unwrap_or("Error");
            let value = output.get("evalue").and_then(Value::as_str).unwrap_or("");
            Some(CellOutput {
                text: clip(&format!("{name}: {value}\n{traceback}")),
                image: None,
            })
        }
        _ => None,
    }
}

/// 渲染单元格源码，非 code 单元格标出类型，非 Python 代码标出语言。
///
/// 参数:
/// - `cell`: 单元格
///
/// 返回:
/// - `<cell id="...">...</cell id="...">` 文本
fn render_cell(cell: &NotebookCell) -> String {
    let mut metadata = String::new();
    if cell.cell_type != "code" {
        metadata.push_str(&format!("<cell_type>{}</cell_type>", cell.cell_type));
    }
    if let Some(language) = cell.language.as_deref().filter(|value| *value != "python") {
        metadata.push_str(&format!("<language>{language}</language>"));
    }
    format!(
        "<cell id=\"{id}\">{metadata}{source}</cell id=\"{id}\">",
        id = cell.id,
        source = cell.source
    )
}

/// 把输出中的 base64 图片处理成模型附件。
///
/// 参数:
/// - `path`: notebook 路径
/// - `index`: 单元格下标
/// - `media_type`: 输出声明的 MIME 类型
/// - `data`: base64 数据
///
/// 返回:
/// - 图片附件；解码或处理失败时忽略
fn output_image(
    path: &Path,
    index: usize,
    media_type: &str,
    data: &str,
) -> Option<ToolModelAttachment> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .ok()
        .filter(|bytes| !bytes.is_empty())?;
    let processed = process_image(&bytes, max_output_tokens()).ok()?;
    let source = format!("{}#cell-{index} ({media_type})", path.display());
    let note = image_metadata_text(&source, &processed, bytes.len() as u64);
    Some(ToolModelAttachment::new(processed.data_url(), source, note))
}

/// 把字符串或字符串数组拼成文本。
fn joined_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(items)) => items.iter().filter_map(Value::as_str).collect(),
        _ => String::new(),
    }
}

/// 截断过长的输出文本。
fn clip(text: &str) -> String {
    if text.chars().count() <= OUTPUT_TEXT_MAX_CHARS {
        return text.to_string();
    }
    let kept = text.chars().take(OUTPUT_TEXT_MAX_CHARS).collect::<String>();
    format!("{kept}\n... [output truncated]")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 单元格源码、类型标记与输出文本按 CometixCode 格式拼接。
    #[test]
    fn renders_cells_and_outputs() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("demo.ipynb");
        let notebook = json!({
            "metadata": {"language_info": {"name": "python"}},
            "cells": [
                {"cell_type": "markdown", "id": "intro", "source": ["# Title\n", "text"]},
                {"cell_type": "code", "source": "print(1)", "execution_count": 1,
                 "outputs": [{"output_type": "stream", "text": ["1\n"]}]},
                {"cell_type": "code", "source": "1/0",
                 "outputs": [{"output_type": "error", "ename": "ZeroDivisionError", "evalue": "division by zero", "traceback": ["line"]}]}
            ]
        });
        std::fs::write(&path, notebook.to_string()).unwrap();

        let output = read_notebook(&path).unwrap();

        assert!(output.content.contains(
            "<cell id=\"intro\"><cell_type>markdown</cell_type># Title\ntext</cell id=\"intro\">"
        ));
        assert!(output
            .content
            .contains("<cell id=\"cell-1\">print(1)</cell id=\"cell-1\">"));
        assert!(output.content.contains("\n1\n"));
        assert!(output
            .content
            .contains("ZeroDivisionError: division by zero"));
        assert!(output.model_attachments.is_empty());
    }
}
