use super::cell::{HistoryCell, TranscriptMode};
use super::render_cache::RenderCache;
use super::subagent_cell::SubagentCell;
use super::tool_cell::ToolCell;
use super::welcome_cell::WelcomeCell;
use crate::llm::{ChatStreamChunk, ChatStreamKind, ToolCallStreamProgress};
use crate::render::tool_view::ToolView;
use crate::render::work_status::WorkStatus;
use crate::render::{ReasoningDisplayMode, ToolCallDisplayMode};
use std::time::Instant;

/// REPL transcript 的渲染选项快照。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TranscriptRenderOptions {
    pub(crate) reasoning_mode: ReasoningDisplayMode,
    pub(crate) tool_call_mode: ToolCallDisplayMode,
}

/// 仍在生成中的文本 source。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LiveTail {
    pub(super) kind: ChatStreamKind,
    pub(super) source: String,
}

/// 正在接收参数的工具调用预览。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LiveToolCall {
    pub(super) name: String,
    pub(super) arguments_preview: String,
}

impl LiveTail {
    /// 将临时流式文本转换为可重放 history cell。
    ///
    /// 参数:
    /// - `self`: 当前临时流式文本
    ///
    /// 返回:
    /// - 对应的历史 cell
    fn into_cell(self) -> HistoryCell {
        match self.kind {
            ChatStreamKind::Content => HistoryCell::markdown(self.source),
            ChatStreamKind::Reasoning => HistoryCell::reasoning(self.source),
        }
    }
}

/// 保存 REPL 会话的定稿 cell 与可变流式尾部。
pub(crate) struct TranscriptStore {
    pub(super) cells: Vec<HistoryCell>,
    pub(super) live_tail: Option<LiveTail>,
    pub(super) live_tool_call: Option<LiveToolCall>,
    pub(super) live_animation_frame: usize,
    pub(super) active_tool_index: Option<usize>,
    pub(super) work_status: Option<WorkStatus>,
    pub(super) work_status_started: Option<Instant>,
    pub(super) row_cap: usize,
    pub(super) cache: RenderCache,
    pub(super) dirty_from_cell: Option<usize>,
}

impl TranscriptStore {
    /// 创建空 transcript。
    ///
    /// 参数:
    /// - `row_cap`: resize 重放时保留的最大视觉行数
    ///
    /// 返回:
    /// - 空 transcript
    pub(crate) fn new(row_cap: usize) -> Self {
        Self {
            cells: Vec::new(),
            live_tail: None,
            live_tool_call: None,
            live_animation_frame: 0,
            active_tool_index: None,
            work_status: None,
            work_status_started: None,
            row_cap: row_cap.max(1),
            cache: RenderCache::default(),
            dirty_from_cell: None,
        }
    }

    /// 更新 row cap。
    ///
    /// 参数:
    /// - `row_cap`: resize 重放时保留的最大视觉行数
    ///
    /// 返回:
    /// - 无
    pub(crate) fn set_row_cap(&mut self, row_cap: usize) {
        self.row_cap = row_cap.max(1);
    }

    /// 返回 resize 重放窗口的行数上限。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 行数上限
    pub(crate) fn row_cap(&self) -> usize {
        self.row_cap
    }

    /// 标记指定 cell 的渲染结果已失效。
    ///
    /// 参数:
    /// - `index`: cell 下标
    ///
    /// 返回:
    /// - 无
    fn mark_dirty(&mut self, index: usize) {
        self.cache.invalidate(index);
        self.dirty_from_cell = Some(match self.dirty_from_cell {
            Some(existing) => existing.min(index),
            None => index,
        });
    }

    /// 清除脏水位，由增量同步在消费视图后调用。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    pub(crate) fn clear_dirty(&mut self) {
        self.dirty_from_cell = None;
    }

    /// 标记所有子智能体单元需要重绘（后台时间线更新时调用）。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    pub(crate) fn mark_subagents_dirty(&mut self) {
        let indexes = self
            .cells
            .iter()
            .enumerate()
            .filter(|(_, cell)| matches!(cell, HistoryCell::Tool(ToolCell::Subagent(_))))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        for index in indexes {
            self.mark_dirty(index);
        }
    }

    /// 记录用户输入回显。
    ///
    /// 参数:
    /// - `mode`: 用户提交时的 REPL 模式
    /// - `text`: 原始输入文本
    ///
    /// 返回:
    /// - 无
    pub(crate) fn push_user_echo(&mut self, mode: TranscriptMode, text: String) {
        self.push_cell(HistoryCell::user_echo(mode, text));
    }

    /// 记录 Sai 主动提交的自动输入回显。
    ///
    /// 参数:
    /// - `text`: 展示给用户的自动消息文本
    ///
    /// 返回:
    /// - 无
    pub(crate) fn push_automatic_echo(&mut self, text: String) {
        self.push_user_echo(TranscriptMode::Automatic, text);
    }

    /// 记录系统提示或控制命令输出。
    ///
    /// 参数:
    /// - `text`: 原始消息文本
    ///
    /// 返回:
    /// - 无
    pub(crate) fn push_meta(&mut self, text: String) {
        self.push_cell(HistoryCell::meta(text));
    }

    /// 记录等待用户选择的权限事件。
    ///
    /// 参数:
    /// - `request`: 权限请求
    ///
    /// 返回:
    /// - 无
    pub(crate) fn push_permission_request(
        &mut self,
        request: crate::permission::PermissionRequest,
    ) {
        self.finalize_live_tail();
        // 优先挂到当前活动工具；若活动索引已漂移，回退扫描最近匹配工具，避免审计 UI 丢失
        if let Some(index) = self.active_tool_index {
            if self.attach_permission_at(index, &request) {
                return;
            }
        }
        for index in (0..self.cells.len()).rev() {
            if self.attach_permission_at(index, &request) {
                self.active_tool_index = Some(index);
                return;
            }
        }
    }

    /// 更新指定权限事件的最终决定。
    ///
    /// 参数:
    /// - `request_id`: 权限请求标识
    /// - `decision`: 用户决定
    ///
    /// 返回:
    /// - 是否找到并更新了权限事件
    pub(crate) fn resolve_permission(
        &mut self,
        request_id: &str,
        decision: crate::permission::PermissionDecision,
    ) -> bool {
        self.update_permission_cells(|cell| match cell {
            HistoryCell::Tool(ToolCell::Invocation(view)) => {
                view.resolve_permission(request_id, decision.clone())
            }
            HistoryCell::Diff(cell) => cell.resolve_permission(request_id, decision.clone()),
            _ => false,
        })
    }

    /// 更新指定权限事件的拒绝回复草稿。
    ///
    /// 参数:
    /// - `request_id`: 权限请求标识
    /// - `draft`: 回复草稿；空值表示返回权限选择
    ///
    /// 返回:
    /// - 是否找到并更新了权限事件
    pub(crate) fn set_permission_reply_draft(
        &mut self,
        request_id: &str,
        draft: Option<String>,
    ) -> bool {
        self.update_permission_cells(|cell| match cell {
            HistoryCell::Tool(ToolCell::Invocation(view)) => {
                view.set_permission_reply(request_id, draft.clone())
            }
            HistoryCell::Diff(cell) => cell.set_permission_reply(request_id, draft.clone()),
            _ => false,
        })
    }

    /// 更新指定权限事件的当前高亮选项。
    ///
    /// 参数:
    /// - `request_id`: 权限请求标识
    /// - `selected`: 高亮选项
    ///
    /// 返回:
    /// - 是否找到并更新了权限事件
    pub(crate) fn set_permission_choice(
        &mut self,
        request_id: &str,
        selected: crate::render::PermissionChoice,
    ) -> bool {
        self.update_permission_cells(|cell| match cell {
            HistoryCell::Tool(ToolCell::Invocation(view)) => {
                view.set_permission_choice(request_id, selected)
            }
            HistoryCell::Diff(cell) => cell.set_permission_choice(request_id, selected),
            _ => false,
        })
    }

    /// 从最新 cell 向前查找并更新权限事件，命中时标脏。
    ///
    /// 参数:
    /// - `update`: 权限更新函数，命中时返回 true
    ///
    /// 返回:
    /// - 是否找到并更新了权限事件
    fn update_permission_cells<F>(&mut self, mut update: F) -> bool
    where
        F: FnMut(&mut HistoryCell) -> bool,
    {
        for index in (0..self.cells.len()).rev() {
            if update(&mut self.cells[index]) {
                self.mark_dirty(index);
                return true;
            }
        }
        false
    }

    /// 记录 REPL 本地 Shell 命令与输出。
    ///
    /// 参数:
    /// - `command`: Shell 命令
    /// - `output`: 命令输出
    /// - `exit_code`: 可选退出码
    ///
    /// 返回:
    /// - 无
    pub(crate) fn push_shell(&mut self, command: String, output: String, exit_code: Option<i32>) {
        self.push_cell(HistoryCell::shell(command, output, exit_code));
    }

    /// 记录 REPL 启动信息面板。
    ///
    /// 参数:
    /// - `cell`: 启动信息 source
    ///
    /// 返回:
    /// - 无
    pub(crate) fn push_welcome(&mut self, cell: WelcomeCell) {
        self.push_cell(HistoryCell::welcome(cell));
    }

    /// 记录模型流式文本片段，并在种类变化时收敛旧尾部。
    ///
    /// 参数:
    /// - `chunk`: 模型流式文本片段
    ///
    /// 返回:
    /// - 无
    pub(crate) fn push_chunk(&mut self, chunk: &ChatStreamChunk) {
        match self.live_tail.as_mut() {
            Some(tail) if tail.kind == chunk.kind => tail.source.push_str(&chunk.text),
            Some(_) => {
                self.finalize_live_tail();
                self.live_animation_frame = 0;
                self.live_tail = Some(LiveTail {
                    kind: chunk.kind,
                    source: chunk.text.clone(),
                });
            }
            None => {
                self.live_animation_frame = 0;
                self.live_tail = Some(LiveTail {
                    kind: chunk.kind,
                    source: chunk.text.clone(),
                });
            }
        }
    }

    /// 记录尚未定稿的工具参数流预览。
    ///
    /// 参数:
    /// - `progress`: 当前工具调用参数接收进度
    ///
    /// 返回:
    /// - 无
    pub(crate) fn push_tool_call_progress(&mut self, progress: &ToolCallStreamProgress) {
        self.live_tool_call = Some(LiveToolCall {
            name: progress.name.clone().unwrap_or_else(|| "tool".to_string()),
            arguments_preview: progress.arguments_preview.clone(),
        });
    }

    /// 更新当前单轮工作状态。
    ///
    /// 参数:
    /// - `status`: 新工作状态
    ///
    /// 返回:
    /// - 状态是否发生变化
    pub(crate) fn set_work_status(&mut self, status: WorkStatus) -> bool {
        if self.work_status == Some(status) {
            return false;
        }
        self.work_status = Some(status);
        self.work_status_started = Some(Instant::now());
        true
    }

    /// 清除当前单轮工作状态。
    ///
    /// 返回:
    /// - 是否清除了状态
    pub(crate) fn clear_work_status(&mut self) -> bool {
        self.work_status_started = None;
        self.work_status.take().is_some()
    }

    /// 在追加定稿 cell 前收敛当前流式尾部。
    ///
    /// 参数:
    /// - `cell`: 定稿 history cell
    ///
    /// 返回:
    /// - 无
    pub(crate) fn push_cell(&mut self, cell: HistoryCell) {
        self.finalize_live_tail();
        let index = self.cells.len();
        self.cells.push(cell);
        self.cache.push_slot();
        self.mark_dirty(index);
    }

    /// 记录工具调用。
    ///
    /// 参数:
    /// - `name`: 工具名称
    /// - `arguments`: 原始工具参数
    ///
    /// 返回:
    /// - 无
    pub(crate) fn push_tool_call(&mut self, name: String, arguments: String) {
        self.finalize_live_tail();
        self.live_tool_call = None;
        let index = self.cells.len();
        if name == "edit_file" {
            self.push_cell(HistoryCell::diff(arguments));
        } else if name == "subagent" {
            self.push_cell(HistoryCell::Tool(ToolCell::Subagent(SubagentCell::new(
                arguments,
            ))));
        } else {
            self.push_cell(HistoryCell::Tool(ToolCell::Invocation(ToolView::running(
                name, arguments,
            ))));
        }
        self.active_tool_index = Some(index);
    }

    /// 记录工具结果。
    ///
    /// 参数:
    /// - `name`: 工具名称
    /// - `ok`: 工具是否成功
    /// - `output`: 原始工具输出
    ///
    /// 返回:
    /// - 无
    pub(crate) fn push_tool_result(&mut self, name: String, ok: bool, output: String) {
        self.finalize_live_tail();
        if name == "subagent" && self.update_active_subagent(|cell| cell.finish(ok, output.clone()))
        {
            self.active_tool_index = None;
            return;
        }
        // edit_file 使用 DiffCell：在原 diff 上标记完成，避免再塞一个空结果 cell
        if name == "edit_file" && self.finish_active_diff(ok) {
            self.active_tool_index = None;
            self.work_status = None;
            self.work_status_started = None;
            return;
        }
        if self.update_active_tool(&name, |view| view.finish(ok, output.clone())) {
            self.active_tool_index = None;
            self.work_status = None;
            self.work_status_started = None;
            return;
        }
        let mut view = ToolView::running(name, String::new());
        view.finish(ok, output);
        self.push_cell(HistoryCell::Tool(ToolCell::Invocation(view)));
        self.active_tool_index = None;
        self.work_status = None;
        self.work_status_started = None;
    }

    /// 记录工具进度。
    ///
    /// 参数:
    /// - `name`: 工具名称
    /// - `message`: 原始进度信息
    ///
    /// 返回:
    /// - 无
    pub(crate) fn push_tool_progress(&mut self, name: String, message: String) {
        self.finalize_live_tail();
        if name == "subagent"
            && self.update_active_subagent(|cell| cell.push_progress(message.clone()))
        {
            return;
        }
        if self.update_active_tool(&name, |view| view.set_progress(message.clone())) {
            return;
        }
        let mut view = ToolView::running(name, String::new());
        view.set_progress(message);
        self.push_cell(HistoryCell::Tool(ToolCell::Invocation(view)));
    }

    /// 记录上下文压缩开始事件。
    ///
    /// 参数:
    /// - `turn_count`: 待压缩的轮次数
    /// - `model`: 压缩模型标签
    ///
    /// 返回:
    /// - 无
    pub(crate) fn push_compaction_started(&mut self, turn_count: usize, model: String) {
        self.active_tool_index = None;
        self.push_cell(HistoryCell::Tool(ToolCell::CompactionStarted {
            turn_count,
            model,
        }));
    }

    /// 记录上下文压缩结束事件。
    ///
    /// 参数:
    /// - `applied`: 是否成功应用压缩
    /// - `message`: 失败概要
    /// - `detail`: 失败详情
    ///
    /// 返回:
    /// - 无
    pub(crate) fn push_compaction_finished(
        &mut self,
        applied: bool,
        message: Option<String>,
        detail: Option<String>,
    ) {
        self.active_tool_index = None;
        self.push_cell(HistoryCell::Tool(ToolCell::CompactionFinished {
            applied,
            message,
            detail,
        }));
    }

    /// 将当前流式尾部收敛为定稿 cell。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 是否发生收敛
    pub(crate) fn finalize_live_tail(&mut self) -> bool {
        let cleared_tool_preview = self.live_tool_call.take().is_some();
        let Some(tail) = self.live_tail.take() else {
            return cleared_tool_preview;
        };
        if tail.source.is_empty() {
            return cleared_tool_preview;
        }
        let index = self.cells.len();
        self.cells.push(tail.into_cell());
        self.cache.push_slot();
        self.mark_dirty(index);
        true
    }

    /// 清空当前 REPL 会话的所有 source cell。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    pub(crate) fn clear(&mut self) {
        self.cells.clear();
        self.live_tail = None;
        self.live_tool_call = None;
        self.live_animation_frame = 0;
        self.active_tool_index = None;
        self.cache.clear();
        self.dirty_from_cell = None;
    }

    /// 结束当前活动的 edit_file Diff 单元。
    ///
    /// 参数:
    /// - `ok`: 编辑是否成功
    ///
    /// 返回:
    /// - 是否命中 Diff cell
    fn finish_active_diff(&mut self, ok: bool) -> bool {
        let Some(index) = self.active_tool_index else {
            return false;
        };
        let Some(HistoryCell::Diff(cell)) = self.cells.get_mut(index) else {
            return false;
        };
        cell.finish(ok);
        self.mark_dirty(index);
        true
    }

    /// 将权限审计挂到指定 transcript 单元。
    ///
    /// 参数:
    /// - `index`: cell 索引
    /// - `request`: 权限请求
    ///
    /// 返回:
    /// - 是否成功附着
    fn attach_permission_at(
        &mut self,
        index: usize,
        request: &crate::permission::PermissionRequest,
    ) -> bool {
        let attached = match self.cells.get_mut(index) {
            Some(HistoryCell::Tool(ToolCell::Invocation(view))) if view.name == request.tool => {
                view.request_permission(request.id.clone());
                true
            }
            Some(HistoryCell::Diff(cell)) if request.tool == "edit_file" => {
                cell.request_permission(request.id.clone());
                true
            }
            _ => false,
        };
        if attached {
            self.mark_dirty(index);
        }
        attached
    }

    /// 更新当前活动工具单元。
    ///
    /// 参数:
    /// - `name`: 工具名称
    /// - `update`: 工具视图更新函数
    ///
    /// 返回:
    /// - 是否找到可更新的活动工具
    fn update_active_tool<F>(&mut self, name: &str, update: F) -> bool
    where
        F: FnOnce(&mut ToolView),
    {
        let Some(index) = self.active_tool_index else {
            return false;
        };
        let Some(HistoryCell::Tool(ToolCell::Invocation(view))) = self.cells.get_mut(index) else {
            return false;
        };
        if !view.is_active_for(name) {
            return false;
        }
        update(view);
        self.mark_dirty(index);
        true
    }

    /// 更新当前活动的子智能体单元。
    ///
    /// 参数:
    /// - `update`: 子智能体单元更新函数
    ///
    /// 返回:
    /// - 是否找到仍在执行的子智能体
    fn update_active_subagent<F>(&mut self, update: F) -> bool
    where
        F: FnOnce(&mut SubagentCell),
    {
        let Some(index) = self.active_tool_index else {
            return false;
        };
        let Some(HistoryCell::Tool(ToolCell::Subagent(cell))) = self.cells.get_mut(index) else {
            return false;
        };
        if !cell.is_active() {
            return false;
        }
        update(cell);
        self.mark_dirty(index);
        true
    }

    /// 推进 live reasoning 的跳动帧。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 是否存在需要刷新的 reasoning live tail
    pub(crate) fn advance_live_animation(&mut self) -> bool {
        let has_reasoning = self
            .live_tail
            .as_ref()
            .is_some_and(|tail| tail.kind == ChatStreamKind::Reasoning && !tail.source.is_empty());
        let has_work_status = self.work_status.is_some();
        if !has_reasoning && !has_work_status {
            return false;
        }
        self.live_animation_frame = self.live_animation_frame.wrapping_add(1);
        true
    }

    /// 判断 transcript 中是否有仍在更新的后台子智能体。
    ///
    /// 返回:
    /// - 需要定时重绘时返回 true
    pub(crate) fn has_running_subagents(&self) -> bool {
        self.cells.iter().any(|cell| {
            matches!(
                cell,
                HistoryCell::Tool(ToolCell::Subagent(subagent)) if subagent.has_live_updates()
            )
        })
    }

    /// 返回 transcript 中子智能体状态和时间线签名。
    ///
    /// 返回:
    /// - 按 cell 顺序组织的状态签名
    pub(crate) fn subagent_signature(&self) -> Vec<(String, String, u64, u64)> {
        self.cells
            .iter()
            .filter_map(|cell| match cell {
                HistoryCell::Tool(ToolCell::Subagent(subagent)) => subagent.state_signature(),
                _ => None,
            })
            .collect()
    }
}
