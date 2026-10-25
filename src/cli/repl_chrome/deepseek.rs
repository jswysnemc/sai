use super::ReplChrome;
use crate::render::clip_to_width;

/// 【终端】【简洁状态】按信息优先级组成状态栏，窄终端先省略思考等级。
/// 参数: `chrome` 为当前状态，`cols` 为宽度；返回: 左侧纯文本状态。
pub(super) fn footer_left(chrome: &ReplChrome, cols: usize) -> String {
    let primary = format!(
        "{}  {}  {}",
        chrome.mode_plain(),
        chrome.context_status(),
        chrome.model
    );
    let detailed = format!("{primary}  {}", chrome.thinking);
    clip_to_width(
        if cols < 60 { &primary } else { &detailed },
        cols,
        if cols >= 3 { "..." } else { "" },
    )
}

#[cfg(test)]
#[path = "deepseek_tests.rs"]
mod tests;
