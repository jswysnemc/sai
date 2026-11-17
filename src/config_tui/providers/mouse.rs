//! 【供应商配置】【鼠标调度】管理三栏浏览器悬停、滚轮缩放、点击切换与底栏快捷动作。

use anyhow::Result;
use crossterm::event::{KeyCode, MouseButton, MouseEvent, MouseEventKind};
use std::io;

use super::ProviderBrowser;
use crate::config_tui::layout::{scroll_start, FrameRect};
use crate::config_tui::mouse::{scroll_index, ClickTracker, HitButtons, HitRect, ListHitbox};

/// 供应商浏览器可点击区域枚举。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProviderClick {
    FilterBar,
    ColumnHeader(usize),
    Row(usize, usize),
    Footer(KeyCode),
}

/// 供应商浏览器帧内各栏与控件的鼠标命中几何。
#[derive(Clone, Debug, Default)]
pub(super) struct ProviderHitbox {
    pub frame: HitRect,
    pub filter_bar: HitRect,
    pub headers: [Option<HitRect>; 3],
    pub columns: [Option<ListHitbox>; 3],
    pub footer: HitButtons<KeyCode>,
}

/// 鼠标事件处理后的外层循环动作。
pub(super) enum BrowserMouseStep {
    None,
    ForceRedraw,
    Key(KeyCode),
}

/// 【供应商配置】【栏命中区】生成单栏标题栏与条目区的命中几何。
///
/// 参数:
/// - `area`: 栏布局矩形
/// - `selected`: 该栏当前选中下标
/// - `total`: 该栏条目总数
///
/// 返回:
/// - `(标题栏命中区, 列表命中区)`
pub(super) fn column_hitbox(
    area: FrameRect,
    selected: usize,
    total: usize,
) -> (HitRect, ListHitbox) {
    let visible_rows = area.height.saturating_sub(2) as usize;
    let start = scroll_start(selected, visible_rows);
    let header = HitRect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: 1,
    };
    let hitbox = ListHitbox {
        frame: HitRect::from(area),
        list: HitRect {
            x: area.x,
            y: area.y.saturating_add(1),
            width: area.width,
            height: area.height.saturating_sub(2),
        },
        start,
        total,
        visible_rows,
    };
    (header, hitbox)
}

impl ProviderHitbox {
    /// 【供应商配置】【悬停查询】返回指针所在栏与列表条目下标。
    pub(super) fn hover_at(&self, column: u16, row: u16) -> Option<(usize, usize)> {
        self.columns
            .iter()
            .enumerate()
            .find_map(|(col_idx, hitbox)| {
                hitbox
                    .as_ref()?
                    .row_at(column, row)
                    .map(|idx| (col_idx, idx))
            })
    }

    /// 【供应商配置】【栏定位】返回指针当前落在哪一栏区域（含标题与列表）。
    pub(super) fn col_at(&self, column: u16, row: u16) -> Option<usize> {
        self.columns
            .iter()
            .enumerate()
            .find_map(|(col_idx, hitbox)| {
                hitbox
                    .as_ref()?
                    .frame
                    .contains(column, row)
                    .then_some(col_idx)
            })
    }

    /// 【供应商配置】【点击目标】把终端坐标归类为过滤框、栏标题、条目行或底栏按钮。
    pub(super) fn click_target(&self, column: u16, row: u16) -> Option<ProviderClick> {
        if self.filter_bar.contains(column, row) {
            return Some(ProviderClick::FilterBar);
        }
        if let Some(code) = self.footer.hit(column, row) {
            return Some(ProviderClick::Footer(code));
        }
        if let Some((col_idx, item_idx)) = self.hover_at(column, row) {
            return Some(ProviderClick::Row(col_idx, item_idx));
        }
        self.headers.iter().enumerate().find_map(|(col_idx, rect)| {
            rect.as_ref()?
                .contains(column, row)
                .then_some(ProviderClick::ColumnHeader(col_idx))
        })
    }
}

impl ProviderBrowser<'_> {
    /// 【供应商配置】【鼠标调度】处理三栏悬停、滚轮、栏切换与点击编辑。
    ///
    /// 参数:
    /// - `stdout`: 终端输出
    /// - `mouse`: 鼠标输入事件
    /// - `hitbox`: 当前帧命中几何
    /// - `clicks`: 浏览器单击判定状态
    ///
    /// 返回:
    /// - 是否直接执行动作、强刷画面或转交等效键盘动作
    pub(super) fn handle_browser_mouse(
        &mut self,
        stdout: &mut io::Stdout,
        mouse: MouseEvent,
        hitbox: &ProviderHitbox,
        clicks: &mut ClickTracker<ProviderClick>,
    ) -> Result<BrowserMouseStep> {
        // 1. 鼠标移动更新当前悬停目标（列表行、栏标题、过滤条与底栏按键）；签名未变时外层不重绘
        if matches!(mouse.kind, MouseEventKind::Moved) {
            self.hovered = hitbox.click_target(mouse.column, mouse.row);
            self.hovered_footer = hitbox.footer.rect_at(mouse.column, mouse.row);
            return Ok(BrowserMouseStep::None);
        }
        // 2. 滚轮按指针所在栏独立滚动（未聚焦栏也可直接滚轮浏览）
        if matches!(
            mouse.kind,
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
        ) {
            let target_col = hitbox.col_at(mouse.column, mouse.row).or_else(|| {
                hitbox
                    .frame
                    .contains(mouse.column, mouse.row)
                    .then_some(self.active_col)
            });
            if let Some(col) = target_col {
                match col {
                    0 => {
                        if let Some(next) =
                            scroll_index(mouse.kind, self.provider_idx, self.config.providers.len())
                        {
                            self.provider_idx = next;
                            self.refresh_models();
                        }
                    }
                    1 => {
                        if let Some(next) = scroll_index(mouse.kind, self.org_idx, self.orgs.len())
                        {
                            self.org_idx = next;
                            self.rebuild_models();
                        }
                    }
                    _ => {
                        if let Some(next) =
                            scroll_index(mouse.kind, self.model_idx, self.models.len())
                        {
                            self.model_idx = next;
                        }
                    }
                }
            }
            return Ok(BrowserMouseStep::None);
        }
        // 3. 右击模型行直接设为当前默认模型，减少键步
        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Right)) {
            if let Some((2, idx)) = hitbox.hover_at(mouse.column, mouse.row) {
                self.active_col = 2;
                self.model_idx = idx;
                self.set_current_model();
            }
            return Ok(BrowserMouseStep::None);
        }
        // 4. 左键单击：切换聚焦栏/选中行，再次单击选中行触发设置或选中模型
        let target = hitbox.click_target(mouse.column, mouse.row);
        let Some(clicked) = clicks.update(mouse, target) else {
            return Ok(BrowserMouseStep::None);
        };
        match clicked {
            ProviderClick::FilterBar => {
                self.enter_filter_mode();
                Ok(BrowserMouseStep::None)
            }
            ProviderClick::ColumnHeader(col) => {
                self.active_col = col.min(2);
                Ok(BrowserMouseStep::None)
            }
            ProviderClick::Footer(code) => Ok(BrowserMouseStep::Key(code)),
            ProviderClick::Row(col, idx) => {
                self.leave_filter_mode();
                let same_cell = self.active_col == col
                    && match col {
                        0 => self.provider_idx == idx,
                        1 => self.org_idx == idx,
                        _ => self.model_idx == idx,
                    };
                self.active_col = col.min(2);
                match col {
                    0 => {
                        if same_cell {
                            self.select_or_edit(stdout)?;
                            return Ok(BrowserMouseStep::ForceRedraw);
                        }
                        self.provider_idx = idx;
                        self.refresh_models();
                    }
                    1 => {
                        self.org_idx = idx;
                        self.rebuild_models();
                    }
                    _ => {
                        if same_cell {
                            self.select_or_edit(stdout)?;
                            return Ok(BrowserMouseStep::ForceRedraw);
                        }
                        self.model_idx = idx;
                    }
                }
                Ok(BrowserMouseStep::None)
            }
        }
    }
}
