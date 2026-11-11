//! 【终端】【模型鼠标】绘制与命中共享列宽、可视窗口和操作按钮。

use super::state::{PickerColumn, PickerState};
use crate::cli::mouse_menu::{ButtonRow, ClickTracker};
use crate::i18n::text as t;
use anyhow::Result;
use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind,
};
use crossterm::terminal;
use std::io;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Target {
    Focus(PickerColumn),
    Item(PickerColumn, usize),
    Key(KeyCode),
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
    pub(super) fn hit(
        &self,
        state: &PickerState,
        column: u16,
        row: usize,
        tasks: bool,
    ) -> Option<Target> {
        if row == self.rows.saturating_sub(1) {
            return controls(self.width, tasks).hit(column);
        }
        if row == if self.rows < 7 { 0 } else { 2 } {
            return tabs(state, self.width).hit(column);
        }
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

    /// 【终端】【模型事件】参数为状态、单击记录及子任务开关；返回可继续交给键盘逻辑的动作。
    pub(super) fn read(
        &mut self,
        picker: &mut PickerState,
        click: &mut ClickTracker<Target>,
        tasks: bool,
    ) -> Result<Option<KeyEvent>> {
        match event::read()? {
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                click.clear();
                Ok(Some(key))
            }
            Event::Resize(columns, rows) => {
                click.clear();
                self.resize(columns, rows)?;
                Ok(None)
            }
            Event::Mouse(mouse) => {
                let Some(row) = mouse.row.checked_sub(self.anchor).map(usize::from) else {
                    click.clear();
                    return Ok(None);
                };
                let layout = PickerLayout::new(picker, self.width, usize::from(self.rows));
                let target = layout.hit(picker, mouse.column, row, tasks);
                if let Some(target) = click.update(mouse, target) {
                    match target {
                        Target::Key(code) => {
                            return Ok(Some(KeyEvent::new(code, KeyModifiers::NONE)))
                        }
                        Target::Focus(column) => picker.focus(column),
                        Target::Item(column, index) => picker.select(column, index),
                    }
                }
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
                        }
                    }
                }
                Ok(None)
            }
            _ => Ok(None),
        }
    }
}
