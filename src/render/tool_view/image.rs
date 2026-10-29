use super::model::ToolView;
use crate::render::tool_event_line::{tool_event_label_tense, tool_status_line, ToolVerbTense};
use crate::render::ToolCallDisplayMode;
use crate::render::{status_style::ToolHealth, terminal_text as t};
use serde_json::Value;

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
    let mut output = tool_status_line(
        &label,
        "",
        if outcome.ok {
            ToolHealth::Ok
        } else {
            ToolHealth::Err
        },
    );
    if !outcome.ok {
        return Some(format!("{output}\n  └ {}", outcome.output));
    }
    let Some(images) = parse_images(&outcome.output) else {
        return Some(format!(
            "{output}\n  {}",
            t("No saved image path returned", "未返回图片保存路径")
        ));
    };
    output = tool_status_line(
        &format!(
            "{label} · {} {}",
            images.len(),
            t("images saved", "张图片已保存")
        ),
        "",
        ToolHealth::Ok,
    );
    // 1. 【终端】【图片路径】路径始终保留，即使终端支持图像协议也不隐藏保存位置
    for image in &images {
        output.push_str(&format!(
            "\n  {}: {}",
            t("Saved", "已保存"),
            super::safe_text::plain(&image.path)
        ));
    }
    if mode == ToolCallDisplayMode::Full {
        let expanded = crate::render::render_expand::expand_override();
        let width = crate::render::fold_text::terminal_wrap_width().saturating_sub(2);
        // 2. 【终端】【图片预览】默认只显示首张有界缩略图，展开后追加其他图片
        for image in images.iter().take(if expanded { usize::MAX } else { 1 }) {
            output.push('\n');
            output.push_str(&crate::render::image_preview::render(&image.path, width));
        }
        if !expanded && images.len() > 1 {
            output.push_str(&format!(
                "\n  {}",
                crate::render::omitted_line::render_fold_hint(
                    &format!(
                        "{} {}",
                        images.len() - 1,
                        t("more image previews", "张预览已折叠")
                    ),
                    Some(crate::render::omitted_line::FOLD_SHORTCUT)
                )
            ));
        }
    }
    Some(output)
}

#[derive(Debug)]
struct ImageReference {
    path: String,
}

/// 【终端】【图片引用】兼容原生多图结果和旧插件单图路径。
/// 参数: `output` 为工具 JSON；返回: 可用的本地图片引用。
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
