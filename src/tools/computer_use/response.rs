//! 解析原生状态和截图；图片原样交给模型，绝不改变操作坐标的像素空间。

use super::session::WindowTarget;
use crate::tools::{ToolModelAttachment, ToolOutput};
use anyhow::{bail, Context, Result};
use base64::Engine;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::io::Cursor;

const MAX_IMAGE_BASE64: usize = 16 * 1024 * 1024;
const MAX_TEXT_CHARS: usize = 128_000;

/// 完整观察结果与后续动作使用的私有快照证明。
pub(super) struct Observation {
    pub(super) output: ToolOutput,
    pub(super) elements: HashMap<String, String>,
    pub(super) image_size: Option<(u32, u32)>,
    pub(super) capture_id: Option<String>,
}

/// 【桌面操作】【驱动错误】将原生 MCP 错误作为失败返回，不制造成功收据。
/// @param result 为驱动返回的 MCP result
/// @returns 驱动未报告错误时成功
pub(super) fn check_error(result: &Value) -> Result<()> {
    let object = result
        .as_object()
        .context("malformed Computer Use driver response")?;
    if object
        .get("isError")
        .is_some_and(|value| !value.is_boolean())
        || (object.get("content").and_then(Value::as_array).is_none()
            && object.get("structuredContent").is_none_or(Value::is_null))
    {
        bail!("malformed Computer Use driver response; observe again before input");
    }
    if result.get("isError").and_then(Value::as_bool) == Some(true) {
        bail!("computer_use_driver_error: {}", text(result));
    }
    Ok(())
}

/// 【桌面操作】【结果文本】优先保留结构化状态；没有结构化字段时合并文本块。
/// @param result 为原生结果
/// @returns 有界文本，不把图片 base64 混入正文
pub(super) fn text(result: &Value) -> String {
    let rendered = if let Some(structured) = result.get("structuredContent") {
        serde_json::to_string_pretty(structured).unwrap_or_default()
    } else {
        result
            .get("content")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|block| block["type"] == "text")
            .filter_map(|block| block["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n")
    };
    limit_text(&rendered, MAX_TEXT_CHARS)
}

/// 【桌面操作】【结构化结果】读取原生结构化数据，兼容以文本块承载 JSON 的实现。
/// @param result 为原生 MCP 结果
/// @returns 解析后的结构化字段
pub(super) fn structured(result: &Value) -> Result<Value> {
    check_error(result)?;
    if let Some(value) = result.get("structuredContent") {
        return Ok(value.clone());
    }
    serde_json::from_str(&text(result))
        .context("Computer Use driver did not return structured state")
}

/// 【桌面操作】【状态组帧】绑定目标与快照，将可操作元素改成不透明句柄并附上原图。
/// @param result 为驱动结果；target 为请求目标；generation 为元素代号；snapshot_id 为快照标识
/// @returns 状态输出、元素映射和截图证明
pub(super) fn observation(
    result: &Value,
    target: WindowTarget,
    generation: u64,
    snapshot_id: &str,
) -> Result<Observation> {
    let state = structured(result)?;
    let state = state
        .as_object()
        .context("Computer Use state must be an object")?;
    // 1. 【桌面操作】【目标核对】驱动明确返回其他目标时拒绝使用，不能猜测坐标归属
    for (field, expected) in [
        ("pid", u64::from(target.pid)),
        ("window_id", target.window_id),
    ] {
        if let Some(actual) = state.get(field) {
            if actual.as_u64() != Some(expected) {
                bail!(
                    "Computer Use {field} mismatch; discard this state and select the target again"
                );
            }
        }
    }
    let (attachment, image_size) = screenshot(result, state, target, snapshot_id)?;
    let mut text = format!(
        "Computer Use state\napp: {}\nwindow_id: {}\nsnapshot_id: {}\n<app_state>\n",
        target.pid, target.window_id, snapshot_id
    );
    let mut metadata = Map::new();
    for key in [
        "app_name",
        "window_title",
        "truncated",
        "truncation_reason",
        "elements_complete",
        "screenshot_error",
    ] {
        if let Some(value) = state.get(key) {
            metadata.insert(key.into(), value.clone());
        }
    }
    text.push_str(&limit_text(&serde_json::to_string(&metadata)?, 4096));
    text.push('\n');
    // 2. 【桌面操作】【元素句柄】只映射驱动签发的 element_token，不接受原始数字或自造引用
    let mut elements = HashMap::new();
    if let Some(rows) = state.get("elements").and_then(Value::as_array) {
        for (index, element) in rows.iter().take(2000).enumerate() {
            let mut row = Map::new();
            if let Some(token) = element
                .get("element_token")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
            {
                let handle = format!("g{generation}:{index}");
                elements.insert(handle.clone(), token.into());
                row.insert("element_index".into(), json!(handle));
            }
            for key in [
                "role", "label", "value", "enabled", "selected", "actions", "depth",
            ] {
                if let Some(value) = element.get(key) {
                    row.insert(key.into(), value.clone());
                }
            }
            text.push_str(&limit_text(&serde_json::to_string(&row)?, 2048));
            text.push('\n');
            if text.len() > MAX_TEXT_CHARS {
                text.push_str("[state truncated; narrow query before choosing a target]\n");
                break;
            }
        }
    } else if let Some(tree) = state.get("tree_markdown").and_then(Value::as_str) {
        text.push_str(&limit_text(tree, MAX_TEXT_CHARS));
        text.push_str("\nNo snapshot-bound element handles were returned.\n");
    }
    text.push_str("</app_state>\n");
    if let Some((width, height)) = image_size {
        text.push_str(&format!(
            "Screenshot: {width}x{height}; use window-local pixels from this exact image.\n"
        ));
    } else {
        text.push_str("Screenshot unavailable; pixel actions are disabled. Use a returned element handle or inspect OS capture permissions.\n");
    }
    text.push_str("Each input consumes this snapshot. Call get_app_state again after input or any uncertain result.");
    let capture_id = state
        .get("capture_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    Ok(Observation {
        output: ToolOutput::text(text).with_model_attachments(attachment),
        elements,
        image_size,
        capture_id,
    })
}

/// 【桌面操作】【截图校验】检查原图格式、尺寸和元数据，保留驱动输出的全部像素。
/// @param result 为原生结果；state 为状态元数据；target 为窗口目标；snapshot_id 为快照标识
/// @returns 可选图片附件及原始尺寸
fn screenshot(
    result: &Value,
    state: &Map<String, Value>,
    target: WindowTarget,
    snapshot_id: &str,
) -> Result<(Option<ToolModelAttachment>, Option<(u32, u32)>)> {
    let images: Vec<_> = result
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|block| block["type"] == "image")
        .collect();
    if images.len() > 1 {
        bail!("Computer Use state contains multiple images; their coordinate target is ambiguous");
    }
    let Some(image) = images.first() else {
        return Ok((None, None));
    };
    let mime = image["mimeType"]
        .as_str()
        .context("Computer Use screenshot is missing mimeType")?;
    let data = image["data"]
        .as_str()
        .context("Computer Use screenshot is missing data")?;
    if data.len() > MAX_IMAGE_BASE64 {
        bail!("Computer Use screenshot exceeds the image size limit");
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .context("invalid Computer Use screenshot base64")?;
    let reader = image::ImageReader::new(Cursor::new(&bytes)).with_guessed_format()?;
    let format = reader
        .format()
        .context("unknown Computer Use screenshot format")?;
    if !matches!(
        (mime, format),
        ("image/png", image::ImageFormat::Png) | ("image/jpeg", image::ImageFormat::Jpeg)
    ) {
        bail!("Computer Use screenshot MIME type does not match its bytes");
    }
    let (width, height) = reader
        .into_dimensions()
        .context("invalid Computer Use screenshot")?;
    if width == 0 || height == 0 || width.max(height) > 4096 {
        bail!("Computer Use screenshot dimensions are unsafe; observe at a smaller size");
    }
    for (key, expected) in [("screenshot_width", width), ("screenshot_height", height)] {
        if let Some(actual) = state.get(key) {
            if actual.as_u64() != Some(u64::from(expected)) {
                bail!("Computer Use {key} does not match the delivered image");
            }
        }
    }
    let source = format!(
        "computer_use:{}:{}:{snapshot_id}",
        target.pid, target.window_id
    );
    let attachment = ToolModelAttachment::new(
        format!("data:{mime};base64,{data}"),
        &source,
        format!("[Image: {source}, {width}x{height}, unchanged window-local screenshot pixels]"),
    );
    Ok((Some(attachment), Some((width, height))))
}

/// 【桌面操作】【文本限长】按字符截断原生文本，避免切断 UTF-8。
/// @param text 为原文；limit 为字符上限
/// @returns 有界文本及必要的截断提示
fn limit_text(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        text.to_owned()
    } else {
        format!(
            "{}\n[truncated]",
            text.chars().take(limit).collect::<String>()
        )
    }
}
