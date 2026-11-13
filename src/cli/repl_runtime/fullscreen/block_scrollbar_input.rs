//! 工具与思考左侧轨道的鼠标捕获，不与正文选择或折叠共享按下状态。

use super::input::Effect;
use super::wheel_boundary::WheelTarget;
use crate::cli::repl_runtime::ReplRuntime;
use crossterm::event::{Event, MouseButton, MouseEventKind};

pub(super) struct BlockDrag {
    target: WheelTarget,
    top: i64,
    bottom: i64,
}

impl ReplRuntime {
    /// 【终端】【轨道拖动】处理按下、捕获拖动与释放；参数为终端事件，返回已消费的绘制效果
    pub(super) fn block_scrollbar_event(&mut self, event: &Event) -> Option<Effect> {
        let Event::Mouse(mouse) = event else {
            if matches!(event, Event::Resize(..)) {
                self.fullscreen.as_mut()?.state.block_drag = None;
            }
            return None;
        };
        let state = &mut self.fullscreen.as_mut()?.state;
        let layout = state.layout?;
        if state.block_drag.is_none() {
            if mouse.kind != MouseEventKind::Down(MouseButton::Left)
                || !layout.in_body(mouse.row)
                || mouse.column >= layout.content_width
            {
                return None;
            }
            let row = state.scroll + usize::from(mouse.row - layout.body_top);
            let target = state
                .document
                .reasonings
                .iter()
                .find(|region| {
                    row > region.start
                        && row < region.end
                        && state.reasoning.is_scrollable(region.key)
                })
                .map(|region| {
                    (
                        WheelTarget::Reasoning(region.key),
                        region.start + 1,
                        region.end,
                    )
                })
                .or_else(|| {
                    state
                        .document
                        .activities
                        .iter()
                        .find(|region| {
                            region.scrollable
                                && row > region.start
                                && row < region.footer.unwrap_or(region.end)
                        })
                        .map(|region| {
                            (
                                WheelTarget::Activity(region.key),
                                region.start + 1,
                                region.footer.unwrap_or(region.end),
                            )
                        })
                })?;
            let gutter = |index: usize| {
                let text =
                    crate::render::ansi_clip::strip_ansi(state.document.lines.get(index)?.as_str());
                let column = text.chars().take_while(|ch| *ch == ' ').count();
                (text.chars().nth(column) == Some('│')).then_some(column as u16)
            };
            if gutter(row) != Some(mouse.column) {
                return None;
            }
            let track: Vec<_> = (target.1..target.2)
                .filter(|index| gutter(*index) == Some(mouse.column))
                .collect();
            let top = *track.first()?;
            let bottom = *track.last()?;
            state.block_drag = Some(BlockDrag {
                target: target.0,
                top: i64::from(layout.body_top) + top as i64 - state.scroll as i64,
                bottom: i64::from(layout.body_top) + bottom as i64 - state.scroll as i64,
            });
            state.press = None;
            state.selection = None;
        }
        if !matches!(
            mouse.kind,
            MouseEventKind::Down(MouseButton::Left)
                | MouseEventKind::Drag(MouseButton::Left)
                | MouseEventKind::Up(MouseButton::Left)
        ) {
            return Some(Effect::Nothing);
        }
        let drag = state.block_drag.as_ref()?;
        let fraction =
            (i64::from(mouse.row) - drag.top) as f64 / (drag.bottom - drag.top).max(1) as f64;
        match drag.target {
            WheelTarget::Activity(key) => {
                state.activity.seek(key, fraction);
            }
            WheelTarget::Reasoning(key) => {
                state.reasoning.seek(key, fraction);
            }
        }
        state.document = state.activity.update(
            state.reasoning.project(),
            usize::from(layout.body_height),
            usize::from(layout.content_width),
            usize::from(layout.rows),
        );
        if mouse.kind == MouseEventKind::Up(MouseButton::Left) {
            state.block_drag = None;
        }
        Some(Effect::Repaint)
    }
}
