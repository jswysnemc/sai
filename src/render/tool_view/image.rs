use super::model::ToolView;
use crate::render::tool_event_line::{tool_event_label_tense, tool_event_text, ToolVerbTense};
use crate::render::ToolCallDisplayMode;
use serde_json::Value;
use std::path::Path;

/// 判断工具是否为专用或旧插件生图工具。
pub(crate) fn is_image_generation_tool(name: &str) -> bool {
    name == "generate_image" || name.ends_with("__generate_image")
}

/// 渲染生图工具的本地终端预览。
///
/// 参数:
/// - view: 工具生命周期数据
/// - mode: 工具展示模式
/// - frame: 动效帧
///
/// 返回:
/// - 生图工具返回 Some；其他工具返回 None
pub(super) fn render(view: &ToolView, mode: ToolCallDisplayMode, _frame: usize) -> Option<String> {
    if !is_image_generation_tool(&view.name) || mode == ToolCallDisplayMode::Hidden {
        return None;
    }
    let label = tool_event_label_tense(
        &view.name,
        Some(&view.arguments),
        ToolVerbTense::from_done(view.outcome.is_some()),
    );
    let Some(outcome) = &view.outcome else {
        return None;
    };
    let status = if outcome.ok { "ok" } else { "err" };
    let mut output = tool_event_text(&label, status);
    if !outcome.ok {
        return Some(format!("{output}\n  └ {}", outcome.output));
    }
    let Some(images) = parse_images(&outcome.output) else {
        return Some(output);
    };
    for image in images {
        let path = Path::new(&image.path);
        match crate::render::terminal_image::render_buffered_image(path, Some("80x30")) {
            Ok(rendered) => {
                output.push('\n');
                output.push_str(&rendered);
            }
            Err(_) => {
                output.push_str(&format!("\n  └ 图片已保存：{}", image.path));
            }
        }
    }
    Some(output)
}

#[derive(Debug)]
struct ImageReference {
    path: String,
}

fn parse_images(output: &str) -> Option<Vec<ImageReference>> {
    let value = serde_json::from_str::<Value>(output).ok()?;
    let is_native = value
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| kind == "image_generation");
    if !is_native && value.get("path").and_then(Value::as_str).is_none() {
        return None;
    }
    let images = value
        .get("images")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let images = images
        .iter()
        .filter_map(|image| {
            image
                .get("local_path")
                .and_then(Value::as_str)
                .filter(|path| !path.trim().is_empty())
                .map(|path| ImageReference {
                    path: path.to_string(),
                })
        })
        .collect::<Vec<_>>();
    if !images.is_empty() {
        return Some(images);
    }
    value
        .get("path")
        .and_then(Value::as_str)
        .filter(|path| !path.trim().is_empty())
        .map(|path| {
            vec![ImageReference {
                path: path.to_string(),
            }]
        })
}
