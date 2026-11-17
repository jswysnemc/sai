//! 【终端】【模型鼠标】绘制与命中共享列宽、可视窗口和操作按钮。

use super::state::{PickerColumn, PickerState};
use crate::cli::mouse_menu::{ButtonRow, ClickTracker};
use crate::i18n::text as t;
use anyhow::Result;
use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind,
};
use crossterm::terminal;
use std::io;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Target {
    FilterBar,
    Focus(PickerColumn),
    Item(PickerColumn, usize),
    Key(KeyCode),
}

/// 选择器单轮读取到的输入事件。
#[derive(Debug, PartialEq, Eq)]
pub(super) enum PickerInput {
    Key(KeyEvent),
    Paste(String),
}

/// 鼠标事件处理后的界面反馈动作。
#[derive(Debug, PartialEq, Eq)]
pub(super) enum MouseAction {
    /// 无可见状态变化，保持当前帧
    Ignore,
    /// 悬停、焦点或选中状态更新，需要重绘面板
    Redraw,
    /// 触发按键等价输入（如确认、取消、切换子任务）
    Input(PickerInput),
}

pub(super) struct PickerLayout {
    pub(super) width: usize,
    pub(super) rows: usize,
    pub(super) widths: [usize; 3],
    pub(super) visible: usize,
    pub(super) content_start: usize,
    pub(super) starts: [usize; 3],
}

impl PickerLayout {
    /// 【终端】【模型布局】参数为状态、宽高；返回各列窗口，短终端保持当前选项可见。
    pub(super) fn new(state: &PickerState, width: usize, rows: usize) -> Self {
        let visible = rows.saturating_sub(if rows < 7 { 2 } else { 6 });
        let start = |index: usize, count: usize| {
            index
                .saturating_sub(visible / 2)
                .min(count.saturating_sub(visible))
        };
        Self {
            width,
            rows,
            visible,
            content_start: if rows < 7 { 1 } else { 4 },
            widths: super::render::column_widths(state, width),
            starts: [
                state.model_window(visible).0,
                start(state.level_index(), state.levels().len()),
                start(
                    state
                        .context_selection()
                        .map_or(0, |context| context.index()),
                    super::context::OPTION_COUNT,
                ),
            ],
        }
    }

    /// 【终端】【列命中】参数为显示列；返回当前可见列，列间留白不命中。
    fn column(&self, column: u16) -> Option<PickerColumn> {
        let mut left = 0;
        for (index, width) in self.widths.iter().copied().enumerate() {
            if width == 0 {
                continue;
            }
            if (left..left + width).contains(&usize::from(column)) {
                return Some(
                    [
                        PickerColumn::Model,
                        PickerColumn::Thinking,
                        PickerColumn::Context,
                    ][index],
                );
            }
            left += width + super::render::COLUMN_GAP;
        }
        None
    }

    /// 【终端】【模型命中】参数为状态、相对坐标及子任务入口开关；返回实际可见的按钮或候选。
    ///
    /// 参数:
    /// - `state`: 当前模型选择状态
    /// - `column`: 相对面板左边缘的列坐标
    /// - `row`: 相对面板锚点的行坐标
    /// - `tasks`: 是否在底栏展示子任务按钮
    ///
    /// 返回:
    /// - 命中的交互控件目标，空白处返回空
    pub(super) fn hit(
        &self,
        state: &PickerState,
        column: u16,
        row: usize,
        tasks: bool,
    ) -> Option<Target> {
        if row >= self.rows || usize::from(column) >= self.width {
            return None;
        }
        // 1. 【模型选择】【固定栏命中】底栏按钮、过滤栏与页签栏按显示区间优先匹配
        if row == self.rows.saturating_sub(1) {
            return controls(self.width, tasks).hit(column);
        }
        if self.rows >= 7 && row == 1 {
            return Some(Target::FilterBar);
        }
        if row == if self.rows < 7 { 0 } else { 2 } {
            return tabs(state, self.width).hit(column);
        }
        // 2. 【模型选择】【列内命中】列标题切换焦点，内容窗口映射为实际候选下标
        let focused = self.column(column)?;
        if self.rows >= 7 && row == 3 {
            return Some(Target::Focus(focused));
        }
        if !(self.content_start..self.content_start + self.visible).contains(&row) {
            return None;
        }
        let index = self.starts[focused as usize] + row - self.content_start;
        let valid = match focused {
            PickerColumn::Model => index < state.models().len(),
            PickerColumn::Thinking => index < state.levels().len(),
            PickerColumn::Context => state
                .context_selection()
                .and_then(|context| context.label(index))
                .is_some(),
        };
        valid.then_some(Target::Item(focused, index))
    }
}

/// 【终端】【模型页签】参数为状态与列数；返回各列入口，窄屏仍可切换隐藏列。
pub(super) fn tabs(state: &PickerState, width: usize) -> ButtonRow<Target> {
    let mut buttons = vec![
        (
            t("Model", "模型").into(),
            Target::Focus(PickerColumn::Model),
        ),
        (
            t("Thinking", "思考").into(),
            Target::Focus(PickerColumn::Thinking),
        ),
    ];
    if state.context_enabled() {
        buttons.push((
            t("Context", "上下文").into(),
            Target::Focus(PickerColumn::Context),
        ));
    }
    ButtonRow::new(buttons, width)
}

/// 【终端】【模型按钮】参数为列数与子任务入口开关；返回保存、取消及可选子任务按钮。
pub(super) fn controls(width: usize, tasks: bool) -> ButtonRow<Target> {
    let mut buttons = vec![
        (t("Save", "保存").into(), Target::Key(KeyCode::Enter)),
        (t("Cancel", "取消").into(), Target::Key(KeyCode::Esc)),
    ];
    if tasks {
        buttons.push((t("Subagents", "子任务").into(), Target::Key(KeyCode::Tab)));
    }
    ButtonRow::new(buttons, width).with_hint(
        t(
            "Enter save · Esc cancel · ↑↓ choose",
            "Enter 保存 · Esc 取消 · ↑↓ 选择",
        ),
        width,
    )
}

pub(super) struct PickerFrame {
    pub(super) anchor: u16,
    pub(super) rows: u16,
    pub(super) width: usize,
}

impl PickerFrame {
    /// 【终端】【面板定位】无参数；预留终端允许的区域并返回实际起点与宽高。
    pub(super) fn new() -> Result<Self> {
        let (columns, rows) = terminal::size()?;
        let rows = rows.min(super::render::frame_row_count());
        super::reserve_frame_space(rows)?;
        let (_, cursor_y) = crossterm::cursor::position().unwrap_or((0, rows.saturating_sub(1)));
        Ok(Self {
            anchor: cursor_y.saturating_sub(rows.saturating_sub(1)),
            rows,
            width: usize::from(columns.saturating_sub(1)),
        })
    }

    /// 【终端】【面板缩放】参数为新宽高；清理仍可见的旧区域并更新坐标，返回终端写入结果。
    pub(super) fn resize(&mut self, columns: u16, rows: u16) -> Result<()> {
        super::clear_frame(
            &mut io::stdout(),
            self.anchor.min(rows.saturating_sub(1)),
            self.rows.min(rows.saturating_sub(self.anchor)),
        )?;
        self.rows = rows.min(super::render::frame_row_count());
        self.anchor = self.anchor.min(rows.saturating_sub(self.rows));
        self.width = usize::from(columns.saturating_sub(1));
        Ok(())
    }

    /// 【终端】【模型事件】读取键盘、粘贴、鼠标、失焦与缩放事件并转换为选择器动作。
    ///
    /// 参数:
    /// - `picker`: 选择状态
    /// - `click`: 单击跟踪器
    /// - `tasks`: 是否启用子任务按钮
    ///
    /// 返回:
    /// - 需交由外层处理的键盘或粘贴动作，返回空表示仅需重绘当前帧
    pub(super) fn read(
        &mut self,
        picker: &mut PickerState,
        click: &mut ClickTracker<Target>,
        tasks: bool,
    ) -> Result<Option<PickerInput>> {
        loop {
            match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => {
                    click.clear();
                    picker.set_hovered(None);
                    return Ok(Some(PickerInput::Key(key)));
                }
                Event::Paste(text) => {
                    click.clear();
                    picker.set_hovered(None);
                    return Ok(Some(PickerInput::Paste(text)));
                }
                Event::FocusLost => {
                    click.clear();
                    if picker.set_hovered(None) {
                        return Ok(None);
                    }
                }
                Event::Resize(columns, rows) => {
                    click.clear();
                    picker.set_hovered(None);
                    self.resize(columns, rows)?;
                    return Ok(None);
                }
                Event::Mouse(mouse) => match apply_mouse_event(
                    picker,
                    click,
                    self.anchor,
                    self.rows,
                    self.width,
                    tasks,
                    mouse,
                ) {
                    MouseAction::Ignore => {}
                    MouseAction::Redraw => return Ok(None),
                    MouseAction::Input(input) => return Ok(Some(input)),
                },
                _ => {}
            }
        }
    }
}

/// 【模型选择】【鼠标处理】处理指针悬停、过滤栏聚焦、二次点击确认与列滚轮滚动。
///
/// 参数:
/// - `picker`: 选择状态
/// - `click`: 单击跟踪器
/// - `anchor`: 面板首行终端行号
/// - `frame_rows`: 面板占用总行数
/// - `width`: 面板可用列宽
/// - `tasks`: 是否启用底栏子任务入口
/// - `mouse`: 终端鼠标事件
///
/// 返回:
/// - 忽略、重绘或按键等价输入动作
pub(super) fn apply_mouse_event(
    picker: &mut PickerState,
    click: &mut ClickTracker<Target>,
    anchor: u16,
    frame_rows: u16,
    width: usize,
    tasks: bool,
    mouse: MouseEvent,
) -> MouseAction {
    // 1. 【模型选择】【边界裁剪】指针移出面板高度或宽度时清理悬停与失效点击
    let in_rows = mouse
        .row
        .checked_sub(anchor)
        .map(usize::from)
        .filter(|&row| row < usize::from(frame_rows));
    let Some(row) = in_rows.filter(|_| usize::from(mouse.column) < width) else {
        if mouse.kind != MouseEventKind::Moved {
            click.clear();
        }
        return if picker.set_hovered(None) {
            MouseAction::Redraw
        } else {
            MouseAction::Ignore
        };
    };

    // 2. 【模型选择】【悬停同步】计算当前指针下的目标并更新实时悬停反馈
    let layout = PickerLayout::new(picker, width, usize::from(frame_rows));
    let target = layout.hit(picker, mouse.column, row, tasks);
    let mut changed = picker.set_hovered(target);

    // 3. 【模型选择】【点击分发】已聚焦且已选中的模型二次点击直接确认，其余控件执行聚焦或选择
    if let Some(clicked) = click.update(mouse, target) {
        match clicked {
            Target::Key(code) => {
                return MouseAction::Input(PickerInput::Key(KeyEvent::new(
                    code,
                    KeyModifiers::NONE,
                )));
            }
            Target::FilterBar => {
                let prev = picker.column();
                picker.focus(PickerColumn::Model);
                changed |= picker.column() != prev;
            }
            Target::Focus(column) => {
                let prev = picker.column();
                picker.focus(column);
                changed |= picker.column() != prev;
                if changed {
                    let next_layout = PickerLayout::new(picker, width, usize::from(frame_rows));
                    picker.set_hovered(next_layout.hit(picker, mouse.column, row, tasks));
                }
            }
            Target::Item(column, index) => {
                if column == PickerColumn::Model
                    && picker.column() == PickerColumn::Model
                    && picker.model_index() == index
                    && index < picker.models().len()
                {
                    return MouseAction::Input(PickerInput::Key(KeyEvent::new(
                        KeyCode::Enter,
                        KeyModifiers::NONE,
                    )));
                }
                picker.select(column, index);
                let next_layout = PickerLayout::new(picker, width, usize::from(frame_rows));
                picker.set_hovered(next_layout.hit(picker, mouse.column, row, tasks));
                return MouseAction::Redraw;
            }
        }
        return if changed {
            MouseAction::Redraw
        } else {
            MouseAction::Ignore
        };
    }

    // 4. 【模型选择】【滚轮导航】在内容区内滚动对应列并同步滚动后的新悬停项
    if row >= layout.content_start && row < layout.content_start + layout.visible {
        if let Some(column) = layout.column(mouse.column) {
            if matches!(
                mouse.kind,
                MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
            ) {
                picker.focus(column);
                if mouse.kind == MouseEventKind::ScrollUp {
                    picker.move_up();
                } else {
                    picker.move_down();
                }
                let next_layout = PickerLayout::new(picker, width, usize::from(frame_rows));
                picker.set_hovered(next_layout.hit(picker, mouse.column, row, tasks));
                return MouseAction::Redraw;
            }
        }
    }

    if changed {
        MouseAction::Redraw
    } else {
        MouseAction::Ignore
    }
}
