//! 【配置界面】【鼠标支持】共享命中区域、滚轮滚动与单击/悬停追踪逻辑。

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};

use super::input::{read_event, ConfigEvent};
use super::layout::FrameRect;
pub(crate) use crate::cli::mouse_menu::ClickTracker;

/// 屏幕上的矩形命中区域。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct HitRect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl HitRect {
    /// 【配置界面】【命中判定】判断终端坐标是否落在本矩形内。
    ///
    /// 参数:
    /// - `column`: 终端列坐标
    /// - `row`: 终端行坐标
    ///
    /// 返回:
    /// - 命中时返回 true
    pub(crate) fn contains(self, column: u16, row: u16) -> bool {
        column >= self.x
            && column < self.x.saturating_add(self.width)
            && row >= self.y
            && row < self.y.saturating_add(self.height)
    }
}

impl From<FrameRect> for HitRect {
    /// 【配置界面】【矩形转换】把面板外框转换为鼠标命中区域；参数为面板矩形，返回命中矩形。
    fn from(rect: FrameRect) -> Self {
        Self {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        }
    }
}

/// 列表与详情区域的鼠标命中几何。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ListHitbox {
    pub frame: HitRect,
    pub list: HitRect,
    pub start: usize,
    pub total: usize,
    pub visible_rows: usize,
}

impl ListHitbox {
    /// 【配置界面】【行命中】根据鼠标坐标反查对应的列表条目下标。
    ///
    /// 参数:
    /// - `column`: 终端列坐标
    /// - `row`: 终端行坐标
    ///
    /// 返回:
    /// - 命中的条目下标；超出列表区域或空行时返回空
    pub(crate) fn row_at(&self, column: u16, row: u16) -> Option<usize> {
        if !self.list.contains(column, row) {
            return None;
        }
        let rel = usize::from(row.saturating_sub(self.list.y));
        let index = self.start.saturating_add(rel);
        (index < self.total).then_some(index)
    }

    /// 【配置界面】【滚轮计算】在面板范围内滚动时计算目标选中项下标。
    ///
    /// 参数:
    /// - `kind`: 鼠标事件类型
    /// - `column`: 终端列坐标
    /// - `row`: 终端行坐标
    /// - `selected`: 当前选中下标
    ///
    /// 返回:
    /// - 发生变化时返回新的目标下标
    pub(crate) fn wheel_target(
        &self,
        kind: MouseEventKind,
        column: u16,
        row: u16,
        selected: usize,
    ) -> Option<usize> {
        if self.total == 0 || !self.frame.contains(column, row) {
            return None;
        }
        scroll_index(kind, selected, self.total)
    }
}

/// 离散按钮或胶囊的命中表。
#[derive(Clone, Debug)]
pub(crate) struct HitButtons<T> {
    items: Vec<(HitRect, T)>,
}

impl<T> Default for HitButtons<T> {
    /// 【配置界面】【按钮表】构造空的按钮命中集合；无参数和返回值说明。
    fn default() -> Self {
        Self { items: Vec::new() }
    }
}

impl<T: Copy + Eq> HitButtons<T> {
    /// 【配置界面】【注册按钮】登记一个可点击矩形及其关联动作。
    ///
    /// 参数:
    /// - `rect`: 按钮占据的屏幕矩形
    /// - `action`: 命中时返回的动作值
    ///
    /// 返回:
    /// - 无
    pub(crate) fn push(&mut self, rect: HitRect, action: T) {
        if rect.width > 0 && rect.height > 0 {
            self.items.push((rect, action));
        }
    }

    /// 【配置界面】【按钮命中】查找指针所在位置的按钮动作。
    ///
    /// 参数:
    /// - `column`: 终端列坐标
    /// - `row`: 终端行坐标
    ///
    /// 返回:
    /// - 命中的动作；未命中任何按钮时返回空
    pub(crate) fn hit(&self, column: u16, row: u16) -> Option<T> {
        self.items
            .iter()
            .rev()
            .find(|(rect, _)| rect.contains(column, row))
            .map(|(_, action)| *action)
    }

    /// 【配置界面】【按钮热区】返回指针正悬停的按钮矩形，用于绘制局部悬停高亮。
    ///
    /// 参数:
    /// - `column`: 终端列坐标
    /// - `row`: 终端行坐标
    ///
    /// 返回:
    /// - 命中的矩形范围；未命中任何按钮时返回空
    pub(crate) fn rect_at(&self, column: u16, row: u16) -> Option<HitRect> {
        self.items
            .iter()
            .rev()
            .find(|(rect, _)| rect.contains(column, row))
            .map(|(rect, _)| *rect)
    }
}

/// 列表交互单次鼠标处理结果。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ListMouseOutcome {
    None,
    Redraw,
    Select(usize),
    Activate(usize),
    RightClick(usize),
}

/// 列表视图的悬停与点击状态追踪器。
#[derive(Default)]
pub(crate) struct ListMouseState {
    pub hovered: Option<usize>,
    pub footer_hover: Option<HitRect>,
    clicks: ClickTracker<usize>,
    button_clicks: ClickTracker<usize>,
}

impl ListMouseState {
    /// 【配置界面】【按钮点击】用索引标识追踪离散按钮或底栏动作点击。
    ///
    /// 参数:
    /// - `mouse`: 鼠标输入事件
    /// - `buttons`: 离散按钮命中集合
    ///
    /// 返回:
    /// - 同一按钮上完整左键单击后返回对应动作
    pub(crate) fn track_buttons<T: Copy + Eq>(
        &mut self,
        mouse: MouseEvent,
        buttons: &HitButtons<T>,
    ) -> Option<T> {
        let hit_index = buttons
            .hit(mouse.column, mouse.row)
            .and_then(|action| buttons.items.iter().position(|(_, item)| *item == action));
        let idx = self.button_clicks.update(mouse, hit_index)?;
        buttons.items.get(idx).map(|(_, action)| *action)
    }

    /// 【配置界面】【列表鼠标】处理移动悬停、滚轮和左键/右键点击。
    ///
    /// 参数:
    /// - `mouse`: 鼠标输入事件
    /// - `hitbox`: 当前帧列表命中几何
    /// - `selected`: 当前选中项下标
    /// - `direct_activate`: 为 true 时首次左键直接激活条目；为 false 时先选中、再击激活
    ///
    /// 返回:
    /// - 归一化的列表鼠标反馈动作
    pub(crate) fn handle(
        &mut self,
        mouse: MouseEvent,
        hitbox: &ListHitbox,
        selected: usize,
        direct_activate: bool,
    ) -> ListMouseOutcome {
        // 1. 悬停追踪：仅当悬停行变化时触发重绘，避免无关鼠标移动引发闪烁
        if matches!(mouse.kind, MouseEventKind::Moved) {
            let next = hitbox.row_at(mouse.column, mouse.row);
            if next != self.hovered {
                self.hovered = next;
                return ListMouseOutcome::Redraw;
            }
            return ListMouseOutcome::None;
        }
        // 2. 滚轮滚动：指针在面板外框内即可上下切换选中项
        if let Some(next) = hitbox.wheel_target(mouse.kind, mouse.column, mouse.row, selected) {
            self.clicks.clear();
            return ListMouseOutcome::Select(next);
        }
        // 3. 右键快捷操作：返回被右击的条目以便菜单触发次级动作
        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Right)) {
            if let Some(index) = hitbox.row_at(mouse.column, mouse.row) {
                return ListMouseOutcome::RightClick(index);
            }
            return ListMouseOutcome::None;
        }
        // 4. 左键单击与再次点击激活
        let target = hitbox.row_at(mouse.column, mouse.row);
        if let Some(index) = self.clicks.update(mouse, target) {
            if direct_activate || index == selected {
                return ListMouseOutcome::Activate(index);
            }
            return ListMouseOutcome::Select(index);
        }
        ListMouseOutcome::None
    }
}

/// 统一菜单循环的输入结果，屏蔽底层鼠标/尺寸事件细节。
#[derive(Clone, Copy, Debug)]
pub(crate) enum MenuInput {
    Key(KeyEvent),
    Select(usize),
    Activate(usize),
    RightClick(usize),
    Action(KeyCode),
    Redraw,
}

/// 【配置界面】【统一读取】等待并把键盘、鼠标与尺寸变更归并为菜单输入。
///
/// 参数:
/// - `state`: 列表鼠标状态追踪器
/// - `hitbox`: 当前帧列表命中区域
/// - `selected`: 当前选中下标
/// - `direct_activate`: 是否允许单击直接触发条目动作
/// - `footer_hits`: 可选底栏快捷键命中表
///
/// 返回:
/// - 归一化菜单输入事件
pub(crate) fn read_menu_input(
    state: &mut ListMouseState,
    hitbox: &ListHitbox,
    selected: usize,
    direct_activate: bool,
    footer_hits: Option<&HitButtons<KeyCode>>,
) -> Result<MenuInput> {
    loop {
        match read_event()? {
            ConfigEvent::Key(key) => return Ok(MenuInput::Key(key)),
            ConfigEvent::Resize => return Ok(MenuInput::Redraw),
            ConfigEvent::Mouse(mouse) => {
                // 1. 先追踪底栏按钮悬停变化，再交给点击判定与列表交互
                let mut footer_redraw = false;
                if matches!(mouse.kind, MouseEventKind::Moved) {
                    let next_footer =
                        footer_hits.and_then(|buttons| buttons.rect_at(mouse.column, mouse.row));
                    if next_footer != state.footer_hover {
                        state.footer_hover = next_footer;
                        footer_redraw = true;
                    }
                }
                if let Some(buttons) = footer_hits {
                    if let Some(code) = state.track_buttons(mouse, buttons) {
                        state.footer_hover = None;
                        return Ok(MenuInput::Action(code));
                    }
                }
                // 2. 分发到列表区域处理悬停、滚轮和行点击
                match state.handle(mouse, hitbox, selected, direct_activate) {
                    ListMouseOutcome::None if footer_redraw => return Ok(MenuInput::Redraw),
                    ListMouseOutcome::None => {}
                    ListMouseOutcome::Redraw => return Ok(MenuInput::Redraw),
                    ListMouseOutcome::Select(index) => return Ok(MenuInput::Select(index)),
                    ListMouseOutcome::Activate(index) => return Ok(MenuInput::Activate(index)),
                    ListMouseOutcome::RightClick(index) => return Ok(MenuInput::RightClick(index)),
                }
            }
        }
    }
}

/// 【配置界面】【滚轮步进】按事件方向与列表长度计算下一选中下标。
///
/// 参数:
/// - `kind`: 鼠标事件类型
/// - `current`: 当前下标
/// - `total`: 列表总长度
///
/// 返回:
/// - 若为滚轮事件且下标发生改变，返回新下标
pub(crate) fn scroll_index(kind: MouseEventKind, current: usize, total: usize) -> Option<usize> {
    if total == 0 {
        return None;
    }
    let step = if total > 24 { 3 } else { 1 };
    let next = match kind {
        MouseEventKind::ScrollUp => current.saturating_sub(step),
        MouseEventKind::ScrollDown => (current.saturating_add(step)).min(total.saturating_sub(1)),
        _ => return None,
    };
    (next != current).then_some(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    /// 构造测试用鼠标事件。
    fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn list_hitbox_resolves_scrolled_rows() {
        let hitbox = ListHitbox {
            frame: HitRect {
                x: 1,
                y: 0,
                width: 80,
                height: 15,
            },
            list: HitRect {
                x: 3,
                y: 3,
                width: 30,
                height: 5,
            },
            start: 4,
            total: 12,
            visible_rows: 5,
        };
        assert_eq!(hitbox.row_at(5, 3), Some(4));
        assert_eq!(hitbox.row_at(10, 7), Some(8));
        assert_eq!(hitbox.row_at(2, 3), None);
        assert_eq!(hitbox.row_at(5, 8), None);
    }

    #[test]
    fn list_mouse_selects_then_activates_selected_row() {
        let hitbox = ListHitbox {
            frame: HitRect {
                x: 0,
                y: 0,
                width: 60,
                height: 12,
            },
            list: HitRect {
                x: 2,
                y: 2,
                width: 24,
                height: 6,
            },
            start: 0,
            total: 4,
            visible_rows: 6,
        };
        let mut state = ListMouseState::default();
        assert_eq!(
            state.handle(mouse(MouseEventKind::Moved, 4, 4), &hitbox, 0, false),
            ListMouseOutcome::Redraw
        );
        assert_eq!(state.hovered, Some(2));
        let _ = state.handle(
            mouse(MouseEventKind::Down(MouseButton::Left), 4, 4),
            &hitbox,
            0,
            false,
        );
        assert_eq!(
            state.handle(
                mouse(MouseEventKind::Up(MouseButton::Left), 4, 4),
                &hitbox,
                0,
                false
            ),
            ListMouseOutcome::Select(2)
        );
        let _ = state.handle(
            mouse(MouseEventKind::Down(MouseButton::Left), 4, 4),
            &hitbox,
            2,
            false,
        );
        assert_eq!(
            state.handle(
                mouse(MouseEventKind::Up(MouseButton::Left), 4, 4),
                &hitbox,
                2,
                false
            ),
            ListMouseOutcome::Activate(2)
        );
    }

    #[test]
    fn wheel_scrolls_even_over_detail_pane() {
        let hitbox = ListHitbox {
            frame: HitRect {
                x: 1,
                y: 0,
                width: 90,
                height: 20,
            },
            list: HitRect {
                x: 3,
                y: 2,
                width: 30,
                height: 10,
            },
            start: 0,
            total: 8,
            visible_rows: 10,
        };
        assert_eq!(
            hitbox.wheel_target(MouseEventKind::ScrollDown, 60, 8, 1),
            Some(2)
        );
        assert_eq!(
            hitbox.wheel_target(MouseEventKind::ScrollUp, 60, 8, 2),
            Some(1)
        );
    }

    #[test]
    fn hit_buttons_returns_matching_action() {
        let mut buttons = HitButtons::default();
        buttons.push(
            HitRect {
                x: 4,
                y: 10,
                width: 8,
                height: 1,
            },
            KeyCode::Enter,
        );
        buttons.push(
            HitRect {
                x: 14,
                y: 10,
                width: 6,
                height: 1,
            },
            KeyCode::Esc,
        );
        assert_eq!(buttons.hit(4, 10), Some(KeyCode::Enter));
        assert_eq!(buttons.hit(15, 10), Some(KeyCode::Esc));
        assert_eq!(buttons.hit(12, 10), None);
        assert_eq!(
            buttons.rect_at(15, 10),
            Some(HitRect {
                x: 14,
                y: 10,
                width: 6,
                height: 1
            })
        );
        assert_eq!(buttons.rect_at(12, 10), None);
    }
}
