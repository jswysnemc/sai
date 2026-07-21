use super::cell::HistoryCell;
use super::store::TranscriptStore;
use super::tool_cell::ToolCell;
use crate::i18n::text as t;

/// 可在 pager 中展开的折叠块内容。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExpandableBlock {
    /// 标题（含类型与对象名）
    pub(crate) title: String,
    /// 完整正文
    pub(crate) body: String,
}

impl TranscriptStore {
    /// 追加当前活动命令的实时输出。
    ///
    /// 参数:
    /// - `name`: 工具名称
    /// - `chunk`: 命令输出片段
    ///
    /// 返回:
    /// - 是否找到活动命令
    pub(crate) fn push_command_output(
        &mut self,
        name: &str,
        chunk: &crate::tools::command::CommandOutputChunk,
    ) -> bool {
        self.update_active_tool(name, |view| {
            view.append_command_output(chunk.stream, &chunk.bytes, chunk.omitted_bytes)
        })
    }

    /// 收集全部可展开块（时间顺序：旧 → 新）。
    ///
    /// 返回:
    /// - 思考段落与命令输出块列表
    pub(crate) fn expandable_blocks(&self) -> Vec<ExpandableBlock> {
        let mut blocks = Vec::new();
        for cell in &self.cells {
            match cell {
                HistoryCell::Reasoning(cell) if !cell.source.trim().is_empty() => {
                    blocks.push(ExpandableBlock {
                        title: t("thinking", "思考").to_string(),
                        body: cell.source.clone(),
                    });
                }
                HistoryCell::Tool(ToolCell::Invocation(view)) if view.has_command_output() => {
                    let body = command_full_body(view);
                    if !body.trim().is_empty() {
                        let label = crate::render::tool_event_line::tool_event_label(
                            &view.name,
                            Some(&view.arguments),
                        );
                        blocks.push(ExpandableBlock {
                            title: format!("{} · {label}", t("command", "命令")),
                            body,
                        });
                    }
                }
                _ => {}
            }
        }
        blocks
    }

    /// 查找最近一个可展开块。
    ///
    /// 返回:
    /// - 标题与完整正文；无则 None
    #[allow(dead_code)]
    pub(crate) fn latest_expandable_block(&self) -> Option<ExpandableBlock> {
        self.expandable_blocks().into_iter().next_back()
    }

    /// 切换最近一个可折叠块的展开状态（兼容测试；TUI 优先走 pager）。
    ///
    /// 返回:
    /// - 找到可切换单元时返回 true
    pub(crate) fn toggle_latest_command_output(&mut self) -> bool {
        // 1. 从后往前找最近的命令输出
        for index in (0..self.cells.len()).rev() {
            let toggled = match self.cells.get_mut(index) {
                Some(HistoryCell::Tool(ToolCell::Invocation(view))) if view.has_command_output() => {
                    view.toggle_command_expanded();
                    true
                }
                _ => false,
            };
            if toggled {
                self.mark_dirty(index);
                return true;
            }
        }
        // 2. 否则切换最近思考段落
        for index in (0..self.cells.len()).rev() {
            if let Some(HistoryCell::Reasoning(cell)) = self.cells.get_mut(index) {
                cell.toggle_expanded();
                self.mark_dirty(index);
                return true;
            }
        }
        false
    }
}

/// 拼装命令工具的完整 stdout/stderr 正文。
///
/// 参数:
/// - `view`: 工具视图
///
/// 返回:
/// - 完整文本
fn command_full_body(view: &crate::render::tool_view::ToolView) -> String {
    let stdout = view.command_stdout_text();
    let stderr = view.command_stderr_text();
    let mut parts = Vec::new();
    if !stdout.is_empty() {
        parts.push(format!("── stdout ──\n{stdout}"));
    }
    if !stderr.is_empty() {
        parts.push(format!("── stderr ──\n{stderr}"));
    }
    if parts.is_empty() {
        if let Some(outcome) = view.outcome.as_ref() {
            return outcome.output.clone();
        }
    }
    parts.join("\n\n")
}
