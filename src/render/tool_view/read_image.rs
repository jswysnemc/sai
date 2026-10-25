use super::ToolView;
use crate::render::{terminal_text as t, ToolCallDisplayMode};

/// 【终端】【读图识别】依据成功的图片结果识别读取，避免误判文本或失败结果。
/// 参数: `view` 为读取工具；返回: 本地图片路径，非读图结果为空。
pub(crate) fn image_read_path(view: &ToolView) -> Option<String> {
    if view.name != "read_file" {
        return None;
    }
    let outcome = view.outcome.as_ref().filter(|outcome| outcome.ok)?;
    let metadata = outcome.output.trim().strip_prefix("[Image: source: ")?;
    let (path, _) = metadata.rsplit_once(", image/")?;
    Some(path.to_string())
}

/// 【终端】【读图预览】只渲染有界图片，不重复文件路径或工具标题。
/// 参数: `view` 为读取工具，`width` 为正文宽度；返回: 图片协议或不可用提示。
pub(crate) fn render_preview(view: &ToolView, width: usize) -> Option<String> {
    let path = image_read_path(view)?;
    Some(crate::render::image_preview::render(&path, width))
}

/// 【终端】【读图卡片】默认折叠图片，显式展开后在左侧展示缩略图。
/// 参数: `view` 为读取工具，`mode` 为展示模式；返回: 图片卡片，非图片读取为空。
pub(super) fn render(view: &ToolView, mode: ToolCallDisplayMode) -> Option<String> {
    let path = image_read_path(view)?;
    let label = format!("{} {path}", t("Viewed image", "已查看图片"));
    let mut body = crate::render::tool_event_line::tool_event_text(&label, "ok");
    if mode != ToolCallDisplayMode::Hidden {
        body.push('\n');
        if crate::render::render_expand::resolve_expanded(false) {
            body.push_str(&crate::render::image_preview::render(
                &path,
                crate::render::fold_text::terminal_wrap_width(),
            ));
        } else {
            body.push_str(&format!(
                "  {}",
                crate::render::omitted_line::render_fold_hint(
                    t("Image preview", "图片预览"),
                    Some(crate::render::omitted_line::FOLD_SHORTCUT)
                )
            ));
        }
    }
    Some(body)
}
