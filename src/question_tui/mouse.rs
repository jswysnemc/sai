//! 【终端提问】【鼠标交互】题签切换、选项聚焦/二次点击确认、确认页提交与长列表滚轮支持。

use super::state::{option_count, QuestionState};
use crate::cli::mouse_menu::ClickTracker;
use crate::question::QuestionRequest;
use anyhow::Result;
use crossterm::event::{MouseEvent, MouseEventKind};
use std::ops::Range;

/// 提问面板可点击的交互目标。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MouseTarget {
    /// 切换到指定题签或确认页
    Tab(usize),
    /// 切换到上一题签
    PrevTab,
    /// 切换到下一题签
    NextTab,
    /// 点击当前问题中的某个选项（含自定义项）
    Option(usize),
    /// 确认页点击提交按钮
    ConfirmSubmit,
}

/// 单次鼠标事件处理后的界面动作。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MouseOutcome {
    /// 未触发状态变更
    None,
    /// 已修改焦点、题签或滚动位置，需重绘
    Redraw,
    /// 已执行单选确认或确认页提交，若回答完整则直接完成提问
    SubmitIfReady,
}

/// 一帧提问面板相对面板锚点的可交互几何映射。
#[derive(Clone, Debug, Default)]
pub(super) struct QuestionHitMap {
    /// 卡片总列宽，限制滚轮与点击的横向范围
    pub(super) card_width: usize,
    /// 面板总行数
    pub(super) panel_rows: usize,
    /// 主体内容可滚动的最大起始行偏移
    pub(super) max_body_scroll: usize,
    /// 按面板相对行号与显示列区间组织的点击命中表
    pub(super) regions: Vec<(usize, Range<usize>, MouseTarget)>,
}

impl QuestionHitMap {
    /// 【终端提问】【命中查询】根据相对面板左上角的行列坐标查找点击目标。
    ///
    /// 参数:
    /// - `row`: 面板内相对行号
    /// - `column`: 终端列号
    ///
    /// 返回:
    /// - 命中的交互目标
    pub(super) fn target_at(&self, row: usize, column: usize) -> Option<MouseTarget> {
        self.regions
            .iter()
            .find(|(hit_row, cols, _)| *hit_row == row && cols.contains(&column))
            .map(|(_, _, target)| *target)
    }

    /// 【终端提问】【框内判定】判断指针是否位于提问卡片矩形区域内。
    ///
    /// 参数:
    /// - `row`: 面板内相对行号
    /// - `column`: 终端列号
    ///
    /// 返回:
    /// - 位于卡片内时返回 true
    pub(super) fn contains(&self, row: usize, column: usize) -> bool {
        row < self.panel_rows && column < self.card_width
    }
}

/// 提问会话期的鼠标跟踪器。
#[derive(Default)]
pub(super) struct QuestionMouse {
    click: ClickTracker<MouseTarget>,
}

impl QuestionMouse {
    /// 【终端提问】【清理点击】在键盘输入或窗口缩放后清除未释放的点击记录。
    ///
    /// 参数:
    /// - `self`: 当前鼠标状态
    ///
    /// 返回:
    /// - 无
    pub(super) fn clear(&mut self) {
        self.click.clear();
    }

    /// 【终端提问】【处理鼠标】结合当前面板锚点与命中映射更新提问状态。
    ///
    /// 参数:
    /// - `mouse`: 终端鼠标事件
    /// - `anchor_y`: 面板在终端中的起始行号
    /// - `hits`: 最近一帧渲染生成的命中表
    /// - `request`: 结构化提问请求
    /// - `state`: 提问交互状态
    ///
    /// 返回:
    /// - 本次鼠标事件对应的后续动作
    pub(super) fn handle(
        &mut self,
        mouse: MouseEvent,
        anchor_y: u16,
        hits: &QuestionHitMap,
        request: &QuestionRequest,
        state: &mut QuestionState,
    ) -> Result<MouseOutcome> {
        let Some(local_row) = mouse.row.checked_sub(anchor_y).map(usize::from) else {
            self.click.clear();
            return Ok(MouseOutcome::None);
        };
        let column = usize::from(mouse.column);
        let target = hits.target_at(local_row, column);

        // 1. 【终端提问】【左键单击】题签切换、选项聚焦与二次点击选定、确认页提交
        if let Some(clicked) = self.click.update(mouse, target) {
            state.cancel_armed_until = None;
            return apply_click(clicked, request, state);
        }

        // 2. 【终端提问】【滚轮滚动】指针位于卡片内时上下滚动长选项列表或确认页摘要
        if hits.contains(local_row, column) {
            match mouse.kind {
                MouseEventKind::ScrollUp => {
                    return Ok(scroll_panel(hits, request, state, false));
                }
                MouseEventKind::ScrollDown => {
                    return Ok(scroll_panel(hits, request, state, true));
                }
                _ => {}
            }
        }
        Ok(MouseOutcome::None)
    }
}

/// 【终端提问】【单击应用】执行已确认的左键单击目标动作。
///
/// 参数:
/// - `target`: 命中的目标
/// - `request`: 结构化提问请求
/// - `state`: 提问交互状态
///
/// 返回:
/// - 状态更新后的动作结果
fn apply_click(
    target: MouseTarget,
    request: &QuestionRequest,
    state: &mut QuestionState,
) -> Result<MouseOutcome> {
    match target {
        MouseTarget::Tab(index) => {
            state.select_tab(request, index);
            Ok(MouseOutcome::Redraw)
        }
        MouseTarget::PrevTab => {
            state.previous_tab(request);
            Ok(MouseOutcome::Redraw)
        }
        MouseTarget::NextTab => {
            state.next_tab(request);
            Ok(MouseOutcome::Redraw)
        }
        MouseTarget::ConfirmSubmit => {
            if !state.on_confirm(request) {
                return Ok(MouseOutcome::None);
            }
            if super::state::submitted_answers(request, state)?.is_some() {
                return Ok(MouseOutcome::SubmitIfReady);
            }
            state.go_to_first_unanswered(request);
            Ok(MouseOutcome::Redraw)
        }
        MouseTarget::Option(index) => {
            if state.on_confirm(request) {
                return Ok(MouseOutcome::None);
            }
            let question = &request.questions[state.tab];
            if index >= option_count(question) {
                return Ok(MouseOutcome::None);
            }
            // 1. 【终端提问】【首次聚焦】点击未聚焦选项时先移动焦点，保留编辑草稿
            if state.selected[state.tab] != index {
                state.leave_editing_preserving_draft();
                state.selected[state.tab] = index;
                return Ok(MouseOutcome::Redraw);
            }
            // 2. 【终端提问】【二次点击】再次点击当前聚焦项即触发编辑、切换勾选或单选确认
            if state.editing {
                return Ok(MouseOutcome::None);
            }
            let is_custom = question.custom && index == question.options.len();
            if is_custom {
                state.activate_current(request)?;
                return Ok(MouseOutcome::Redraw);
            }
            if question.multiple {
                state.toggle_current(request)?;
                return Ok(MouseOutcome::Redraw);
            }
            state.confirm_single(request)?;
            if request.needs_review() {
                Ok(MouseOutcome::Redraw)
            } else {
                Ok(MouseOutcome::SubmitIfReady)
            }
        }
    }
}

/// 【终端提问】【视口滚动】处理滚轮事件，长列表滚动视口，短列表平滑移动选项焦点。
///
/// 参数:
/// - `hits`: 当前帧命中几何与滚动上限
/// - `request`: 结构化提问请求
/// - `state`: 提问交互状态
/// - `down`: 是否向下滚动
///
/// 返回:
/// - 发生位移时返回 `Redraw`
fn scroll_panel(
    hits: &QuestionHitMap,
    request: &QuestionRequest,
    state: &mut QuestionState,
    down: bool,
) -> MouseOutcome {
    if hits.max_body_scroll > 0 {
        let current = state.scroll_starts[state.tab].min(hits.max_body_scroll);
        let next = if down {
            (current + 2).min(hits.max_body_scroll)
        } else {
            current.saturating_sub(2)
        };
        if next != state.scroll_starts[state.tab] {
            state.scroll_starts[state.tab] = next;
            state.free_scroll = true;
            return MouseOutcome::Redraw;
        }
        return MouseOutcome::None;
    }
    if state.on_confirm(request) || state.editing {
        return MouseOutcome::None;
    }
    let question = &request.questions[state.tab];
    let count = option_count(question);
    if count == 0 {
        return MouseOutcome::None;
    }
    let current = state.selected[state.tab];
    let next = if down {
        (current + 1).min(count - 1)
    } else {
        current.saturating_sub(1)
    };
    if next != current {
        state.selected[state.tab] = next;
        MouseOutcome::Redraw
    } else {
        MouseOutcome::None
    }
}
