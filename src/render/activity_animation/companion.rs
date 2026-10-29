//! 底部状态行的左侧引导：会眨眼的表情，替代单列脉冲图标。

mod face;

use super::RESET;

/// 【终端】【状态引导】渲染当前帧的状态表情。
/// 参数：`frame` 为共享时钟帧号；返回：固定五列的 ANSI 文本。
pub(super) fn render_status_companion(frame: usize) -> String {
    // 表情使用默认前景色且不加粗，动感来自眨眼与神情变化
    format!("\x1b[22m\x1b[39m{}{RESET}", face::face_at(frame))
}

/// 【终端】【状态引导】判断终端行是否以状态表情开头，供对齐与区块间距识别状态行。
/// 参数：`text` 为可能带 ANSI 样式的终端行；返回：首个可见内容是否为状态表情。
pub(crate) fn is_status_companion_line(text: &str) -> bool {
    // 普通正文很少包含表情嘴形，先过滤掉以免逐行剥离样式
    if !text.contains(face::MOUTH) {
        return false;
    }
    face::starts_with_face(super::strip_ansi_for_test(text).trim_start())
}

#[cfg(test)]
mod tests;
