//! 子任务时间线转换为共享会话单元，沿用主会话的全屏执行链与折叠交互。

use super::cell::HistoryCell;
use super::fullscreen_parts::ParagraphKey;
use super::fullscreen_view::FullscreenDocument;
use super::markdown_cell::MarkdownCell;
use super::reasoning_cell::ReasoningCell;
use super::store::{TranscriptRenderOptions, TranscriptStore};
use super::subagent_view::{idle_hint, view_heading};
use super::tool_cell::ToolCell;
use crate::i18n::text as t;
use crate::render::tool_view::ToolView;
use crate::tools::subagent_state;
use crate::tools::subagent_timeline::SubagentTimelineEntry;
use std::collections::HashSet;

impl TranscriptStore {
    /// 【终端】【子会话文档】同步后台时间线，使用主会话选项与展开集合渲染。
    /// 参数: id 为子任务标识，label 为名称，width 为净宽，options 为主会话偏好，
    /// expanded 为展开键；返回: 包含执行链、段落命中区域与消息锚点的文档。
    pub(super) fn render_subagent_document(
        &mut self,
        id: &str,
        label: &str,
        width: usize,
        options: &TranscriptRenderOptions,
        expanded: &HashSet<ParagraphKey>,
    ) -> FullscreenDocument {
        let snapshot = subagent_state::subagent_snapshot(id).ok();
        let status = snapshot
            .as_ref()
            .map(|snapshot| crate::render::status_style::subagent_status_key(&snapshot.status))
            .unwrap_or("run");
        let entries = subagent_state::subagent_timeline(id).unwrap_or_default();
        let mut cells = vec![HistoryCell::meta(view_heading(
            label,
            status,
            snapshot.as_ref(),
        ))];
        if entries.is_empty() {
            cells.push(HistoryCell::meta(t("no timeline yet", "暂无时间线").into()));
        }
        // 1. 【终端】【时间线转换】保留条目顺序，工具结果直接绑定对应单元
        for entry in &entries {
            let hidden = options.tool_call_mode == crate::render::ToolCallDisplayMode::Hidden
                && matches!(entry, SubagentTimelineEntry::Tool { ok: None, .. })
                && status != "run";
            cells.push(if hidden {
                HistoryCell::meta(String::new())
            } else {
                entry_cell(entry, status)
            });
        }
        if !entries.iter().any(|entry| {
            matches!(entry, SubagentTimelineEntry::Text { text } if !text.trim().is_empty())
        }) {
            if let Some(result) = snapshot.as_ref().and_then(|snapshot| snapshot.result.as_deref())
                .filter(|text| !text.trim().is_empty()) {
                cells.push(markdown_cell(result));
            }
        }
        if status == "idle" {
            cells.push(HistoryCell::meta(idle_hint()));
        }
        // 2. 【终端】【子会话缓存】缓存独立于主会话，只作废内容发生变化的单元
        if !self
            .subagent_document
            .as_ref()
            .is_some_and(|(cached_id, _)| cached_id == id)
        {
            self.subagent_document =
                Some((id.into(), Box::new(TranscriptStore::new(self.row_cap))));
        }
        let (_, store) = self.subagent_document.as_mut().unwrap();
        let changed = (0..store.cells.len().max(cells.len()))
            .filter(|index| store.cells.get(*index) != cells.get(*index))
            .collect::<Vec<_>>();
        for index in changed {
            store.mark_dirty(index);
        }
        store.cells = cells;
        store.active_tool_index = if status == "run" {
            store.cells.iter().rposition(|cell| {
                matches!(cell, HistoryCell::Tool(ToolCell::Invocation(view)) if view.outcome.is_none())
            })
        } else {
            None
        };
        // 3. 【终端】【共享执行链】工作状态仍由当前子会话的固定面板显示
        store.render_activity_document(width, options, expanded)
    }
}

/// 【终端】【子会话单元】把一条时间线记录转换为共享渲染源。
/// 参数: entry 为时间线条目，status 为任务状态键；返回: 对应会话单元。
fn entry_cell(entry: &SubagentTimelineEntry, status: &str) -> HistoryCell {
    match entry {
        SubagentTimelineEntry::Reasoning { text } => {
            HistoryCell::Reasoning(ReasoningCell::new(text.clone()))
        }
        SubagentTimelineEntry::Text { text } => markdown_cell(text),
        SubagentTimelineEntry::Message { from, text } => {
            let source = if from == "user" {
                t("user message", "用户留言")
            } else {
                t("message from parent agent", "主代理消息")
            };
            HistoryCell::user_echo(super::TranscriptMode::Yolo, format!("{source}\n{text}"))
        }
        SubagentTimelineEntry::Tool {
            name,
            args_preview,
            ok,
            output_preview,
            ..
        } => {
            // 1. 【终端】【终态工具】未收到结果的中断调用保持中性，不伪造成功或失败
            if ok.is_none() && status != "run" {
                let label = crate::render::tool_event_line::tool_event_label_tense(
                    name,
                    Some(args_preview),
                    crate::render::tool_event_line::ToolVerbTense::from_done(true),
                );
                return HistoryCell::meta(crate::render::tool_event_line::tool_event_text(
                    &label, status,
                ));
            }
            let mut view = ToolView::running(name.clone(), args_preview.clone());
            if let Some(ok) = ok {
                view.finish(*ok, output_preview.clone().unwrap_or_default());
            }
            HistoryCell::Tool(ToolCell::Invocation(Box::new(view)))
        }
    }
}

/// 【终端】【子会话正文】创建不附加虚构耗时的正文单元。
/// 参数: text 为 Markdown 原文；返回: 正文单元。
fn markdown_cell(text: &str) -> HistoryCell {
    HistoryCell::Markdown(MarkdownCell {
        source: text.into(),
        table_layouts: Default::default(),
    })
}

#[cfg(test)]
#[path = "subagent_document_tests.rs"]
mod tests;
