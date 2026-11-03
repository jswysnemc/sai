//! 执行链与思考窗口的滚轮路由、边界交接和键盘导航。

use super::state::{ActivityNavigation, FullscreenState};
use super::wheel_boundary::WheelTarget;
use crate::render::transcript::ParagraphKey;

impl FullscreenState {
    /// 【终端】【滚轮路由】命中内部视口时直接滚动，边界先阻挡一次再交给主会话。
    /// 参数: `row` 为命中的正文行，`delta` 为位移，`height` 为正文高度。
    /// 返回: 位置或阅读焦点是否改变。
    pub(super) fn wheel_at(&mut self, row: Option<usize>, delta: isize, height: usize) -> bool {
        if let Some(target) = row.and_then(|row| self.wheel_target_at(row)) {
            let selected = self.activity.selected;
            let changed = self.scroll_wheel_target(target, delta);
            if !self.wheel_boundary.allows_outer(target, delta, changed) {
                return changed || selected != self.activity.selected;
            }
        } else {
            self.wheel_boundary.clear();
        }
        // 1. 【终端】【阅读退出】滚轮交给主会话后清除链内焦点，保留同向边界放行状态
        let cleared = self.activity.selected.take().is_some();
        if cleared && !self.document.activities.is_empty() {
            self.document = self.activity.project();
        }
        self.scroll_by(delta, height) || cleared
    }

    /// 【终端】【指针同步】移动和滚轮事件命中其他区域时重置边界，同一目标内保持状态。
    /// 参数: `row` 为指针命中的正文行，正文外为空；返回: 无。
    pub(super) fn retain_wheel_target_at(&mut self, row: Option<usize>) {
        let target = row.and_then(|row| self.wheel_target_at(row));
        self.wheel_boundary.retain_target(target);
    }

    /// 【终端】【主动阅读】单击正文时选择执行链，普通会话点击退出链内阅读。
    /// 参数: `row` 为被操作的正文行；返回: 阅读焦点是否改变。
    pub(super) fn focus_activity_at(&mut self, row: usize) -> bool {
        let next = self
            .document
            .activities
            .iter()
            .find(|region| region.scrollable && row >= region.start && row < region.end)
            .map(|region| region.key);
        if next == self.activity.selected {
            return false;
        }
        self.activity.selected = next;
        if !self.document.activities.is_empty() {
            self.document = self.activity.project();
        }
        true
    }

    /// 【终端】【链内输入】按鼠标位置滚动内部视口，供测试检查实际位移。
    ///
    /// 参数:
    /// - `row`: 鼠标命中的文档物理行
    /// - `delta`: 滚轮位移
    ///
    /// 返回:
    /// - 命中返回 Some，布尔值仅表示是否位移；不可滚动区域返回 None
    #[cfg(test)]
    pub(super) fn scroll_activity_at(&mut self, row: usize, delta: isize) -> Option<bool> {
        let target = self.wheel_target_at(row)?;
        Some(self.scroll_wheel_target(target, delta))
    }

    /// 【终端】【滚轮命中】优先定位独立思考窗口，再定位可滚动执行链，无须预先选中。
    /// 参数: `row` 为文档物理行；返回: 可滚动内部目标，普通正文返回空值。
    fn wheel_target_at(&self, row: usize) -> Option<WheelTarget> {
        if self.layout.is_some() {
            if let Some(region) = self.document.reasonings.iter().find(|region| {
                row >= region.start && row < region.end && self.reasoning.is_scrollable(region.key)
            }) {
                return Some(WheelTarget::Reasoning(region.key));
            }
        }
        self.document
            .activities
            .iter()
            .find(|region| region.scrollable && row >= region.start && row < region.end)
            .map(|region| WheelTarget::Activity(region.key))
    }

    /// 【终端】【内部滚动】将位移交给已定位的内部视口，不传递剩余位移。
    /// 参数: `target` 为内部目标，`delta` 为行偏移；返回: 内部位置是否改变。
    fn scroll_wheel_target(&mut self, target: WheelTarget, delta: isize) -> bool {
        match target {
            WheelTarget::Activity(key) => self.scroll_activity(key, delta),
            WheelTarget::Reasoning(key) => self.scroll_reasoning(key, delta),
        }
    }

    /// 【终端】【思考滚轮】只改变思考正文位置，其他步骤和输入框保持原位。
    /// 参数: `key` 为思考段落键，`delta` 为偏移；返回: 内部位置是否改变。
    fn scroll_reasoning(&mut self, key: ParagraphKey, delta: isize) -> bool {
        let Some(layout) = self.layout else {
            return false;
        };
        if !self.reasoning.scroll(key, delta) {
            return false;
        }
        self.document = self.activity.update(
            self.reasoning.project(),
            usize::from(layout.body_height),
            usize::from(layout.content_width),
            usize::from(layout.rows),
        );
        self.press = None;
        self.selection = None;
        self.copied = None;
        true
    }

    /// 【终端】【链内键盘】优先浏览已操作的可见链，否则使用最近的可见链。
    /// 参数: `navigation` 为翻页或首尾操作，`body_height` 为正文高度；返回: 是否命中并改变位置。
    pub(super) fn navigate_activity(
        &mut self,
        navigation: ActivityNavigation,
        body_height: usize,
    ) -> Option<bool> {
        let visible = |region: &&crate::render::transcript::ActivityRegion| {
            region.scrollable
                && region.end > self.scroll
                && region.start < self.scroll + body_height
        };
        let key = self
            .document
            .activities
            .iter()
            .filter(visible)
            .find(|region| Some(region.key) == self.activity.selected)
            .or_else(|| self.document.activities.iter().filter(visible).next_back())?
            .key;
        let delta = match navigation {
            ActivityNavigation::Top => isize::MIN,
            ActivityNavigation::Latest => isize::MAX,
            ActivityNavigation::Page(forward) => {
                let page = self.activity.page(key) as isize;
                if forward {
                    page
                } else {
                    -page
                }
            }
        };
        let selected = self.activity.selected;
        let changed = self.scroll_activity(key, delta);
        Some(
            self.reveal_activity(key, body_height) || changed || selected != self.activity.selected,
        )
    }

    /// 【终端】【链内跟随】提示行单击恢复跟随，必要时显示被外层裁切的执行链。
    /// 参数: `row` 为文档行，`body_height` 为正文可用高度；返回: 是否命中并改变位置。
    pub(super) fn resume_activity_at(&mut self, row: usize, body_height: usize) -> Option<bool> {
        let key = self
            .document
            .activities
            .iter()
            .find(|region| region.footer == Some(row))?
            .key;
        let selected = self.activity.selected;
        let changed = self.scroll_activity(key, isize::MAX);
        Some(
            self.reveal_activity(key, body_height) || changed || selected != self.activity.selected,
        )
    }

    /// 【终端】【链内状态】应用独立滚动并清理已失效选区。
    /// 参数: `key` 为链键，`delta` 为偏移；返回: 位置是否改变。
    fn scroll_activity(&mut self, key: ParagraphKey, delta: isize) -> bool {
        let selected = self.activity.selected;
        let changed = self.activity.scroll(key, delta);
        if changed || selected != self.activity.selected {
            self.document = self.activity.project();
            self.press = None;
            self.selection = None;
            self.copied = None;
        }
        changed
    }
}
