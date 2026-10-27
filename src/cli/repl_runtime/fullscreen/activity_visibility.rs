use super::state::FullscreenState;
use crate::render::transcript::ParagraphKey;

impl FullscreenState {
    /// 【终端】【链内可见性】显式键盘导航或恢复跟随时显示被裁切的链，鼠标展开不调用此方法。
    /// 参数: `key` 为执行链键，`body_height` 为正文可用行数；返回: 外层位置是否变化。
    pub(super) fn reveal_activity(&mut self, key: ParagraphKey, body_height: usize) -> bool {
        let Some(region) = self
            .document
            .activities
            .iter()
            .find(|region| region.key == key)
        else {
            return false;
        };
        // 1. 【终端】【范围校验】不为不可滚动或无法完整容纳的区域强行改变外层位置
        if !region.scrollable || region.end.saturating_sub(region.start) > body_height {
            return false;
        }
        // 2. 【终端】【最小位移】已完整可见时保持原位，否则只移动被裁掉的行数
        let target = self
            .scroll
            .clamp(region.end.saturating_sub(body_height), region.start);
        self.scroll_to(target, body_height)
    }
}
