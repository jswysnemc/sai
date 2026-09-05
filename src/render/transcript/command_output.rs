use super::cell::HistoryCell;
use super::store::TranscriptStore;
use super::tool_cell::ToolCell;
use crate::i18n::text as t;
use crate::render::command_result_block::command_result_streams;

pub(crate) use crate::render::expandable::{ExpandableBlock, ExpandableBlockKind};

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
                        title: "thinking".to_string(),
                        body: cell.source.clone(),
                        kind: ExpandableBlockKind::Markdown,
                    });
                }
                HistoryCell::UserEcho(cell) if super::user_echo_cell::would_fold(cell) => {
                    // 粘贴长文本与思考一样进入 Ctrl+O 分页
                    let preview: String = cell.text.chars().take(48).collect();
                    blocks.push(ExpandableBlock {
                        title: format!("{} · {preview}", t("You", "你")),
                        body: crate::render::input_atom::render_input_atoms(
                            &cell.text,
                            &cell.atoms,
                            true,
                        ),
                        kind: ExpandableBlockKind::Plain,
                    });
                }
                HistoryCell::Shell(cell)
                    if !cell.command.trim().is_empty() || !cell.output.trim().is_empty() =>
                {
                    // 1. 标题：完整命令逐行 shell 着色，不省略
                    let colored = colorize_command_lines(&cell.command);
                    // 2. 正文：完整 stdout（及退出码）
                    let mut body = String::new();
                    if !cell.output.trim().is_empty() {
                        body.push_str("── stdout ──\n");
                        body.push_str(&cell.output);
                    }
                    if let Some(code) = cell.exit_code {
                        if !body.is_empty() {
                            body.push_str("\n\n");
                        }
                        body.push_str(&format!("exit_code: {code}"));
                    }
                    blocks.push(ExpandableBlock {
                        title: format!("{} · {}", t("You ran", "已执行"), colored),
                        body,
                        kind: ExpandableBlockKind::Command,
                    });
                }
                HistoryCell::Diff(cell) => {
                    let body = cell.pager_body();
                    if !body.trim().is_empty() {
                        blocks.push(ExpandableBlock {
                            title: format!("{} · {}", t("diff", "差异"), cell.pager_title()),
                            body,
                            kind: ExpandableBlockKind::Diff,
                        });
                    }
                }
                HistoryCell::Tool(ToolCell::Invocation(view)) if view.has_command_output() => {
                    let body = command_full_body(view);
                    if !body.trim().is_empty() {
                        // 命令标题使用完整命令并做 shell 着色，Ctrl+O 界面不省略
                        let label = if view.name == "background_command" {
                            colorize_command_lines(
                                &crate::render::tool_view::background_task_command(view),
                            )
                        } else {
                            crate::render::tool_event_line::tool_command_title_colored(
                                &view.name,
                                Some(&view.arguments),
                            )
                        };
                        blocks.push(ExpandableBlock {
                            title: format!("{} · {label}", t("command", "命令")),
                            body,
                            kind: ExpandableBlockKind::Command,
                        });
                    }
                }
                HistoryCell::Tool(ToolCell::Invocation(view)) => {
                    let body = tool_full_body(view);
                    if !body.is_empty() {
                        blocks.push(ExpandableBlock {
                            title: crate::render::tool_event_line::tool_event_label_tense(
                                &view.name,
                                Some(&view.arguments),
                                crate::render::tool_event_line::ToolVerbTense::from_done(
                                    view.outcome.is_some(),
                                ),
                            ),
                            body,
                            kind: ExpandableBlockKind::Plain,
                        });
                    }
                }
                _ => {}
            }
        }
        if let Some(tail) = self.live_tail.as_ref().filter(|tail| {
            tail.kind == crate::llm::ChatStreamKind::Reasoning && !tail.source.is_empty()
        }) {
            blocks.push(ExpandableBlock {
                title: "Thinking".to_string(),
                body: tail.source.clone(),
                kind: ExpandableBlockKind::Markdown,
            });
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
                Some(HistoryCell::Tool(ToolCell::Invocation(view)))
                    if view.has_command_output() =>
                {
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
        // 3. 再否则切换最近的粘贴长文本回显
        for index in (0..self.cells.len()).rev() {
            if let Some(HistoryCell::UserEcho(cell)) = self.cells.get_mut(index) {
                if super::user_echo_cell::would_fold(cell) {
                    cell.toggle_expanded();
                    self.mark_dirty(index);
                    return true;
                }
            }
        }
        false
    }
}

/// 收集通用工具的完整参数、进度与结果，使折叠载荷都能通过分段视图读取。
///
/// 参数: `view` 为工具生命周期数据
/// 返回: 保留全文的正文，JSON 以可读缩进显示
fn tool_full_body(view: &crate::render::tool_view::ToolView) -> String {
    if view.name == "todo" {
        return crate::render::render_expand::with_expanded_render(|| {
            crate::render::tool_view::render(view, crate::render::ToolCallDisplayMode::Full)
        });
    }
    let mut sections = Vec::new();
    for (label, payload) in [
        ("Arguments", view.arguments.as_str()),
        ("Progress", view.progress.as_deref().unwrap_or_default()),
        (
            "Output",
            view.outcome
                .as_ref()
                .map(|outcome| outcome.output.as_str())
                .unwrap_or_default(),
        ),
    ] {
        if payload.trim().is_empty() {
            continue;
        }
        let formatted = serde_json::from_str::<serde_json::Value>(payload)
            .ok()
            .and_then(|value| serde_json::to_string_pretty(&value).ok())
            .unwrap_or_else(|| payload.to_string());
        sections.push(format!("{label}\n{formatted}"));
    }
    sections.join("\n\n")
}

/// 将命令文本逐行做 shell 语法着色。
///
/// 参数:
/// - `command`: 原始命令（可多行）
///
/// 返回:
/// - ANSI 着色后的完整命令
fn colorize_command_lines(command: &str) -> String {
    command
        .lines()
        .map(|line| crate::render::code_block::highlight_code_line("bash", line))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 拼装命令工具的完整 stdout/stderr 正文。
///
/// 参数:
/// - `view`: 工具视图
///
/// 返回:
/// - 完整文本
fn command_full_body(view: &crate::render::tool_view::ToolView) -> String {
    if let Some(body) = crate::render::tool_view::background_pager_body(view) {
        return body;
    }
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
            if let Some((_success, stdout, stderr)) = command_result_streams(&outcome.output) {
                if !stdout.trim().is_empty() {
                    parts.push(format!("── stdout ──\n{stdout}"));
                }
                if !stderr.trim().is_empty() {
                    parts.push(format!("── stderr ──\n{stderr}"));
                }
                if !parts.is_empty() {
                    return parts.join("\n\n");
                }
            }
            return outcome.output.clone();
        }
    }
    parts.join("\n\n")
}
