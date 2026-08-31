use super::stream_commands::{StreamCommandContext, StreamInputAction};
use super::{ReplRuntime, StreamComposerDraft};
use crate::agent::AgentMode;
use crate::cli::repl_commands::{
    stream_command_disabled_hint, stream_command_policy, StreamCommandPolicy,
};
use crate::cli::repl_windows_paste::WindowsPasteKey;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use std::io::IsTerminal;
use std::time::Duration;

impl ReplRuntime {
    /// 将主 TUI 重新定位到 transcript 最新输出。
    ///
    /// 参数:
    /// - `streaming`: 当前是否处于模型流式输出阶段
    ///
    /// 返回:
    /// - transcript 与 composer 重绘结果
    pub(in crate::cli) fn jump_to_output_bottom(&mut self, streaming: bool) -> Result<()> {
        self.replay(streaming)
    }

    /// 保存模型运行期间收到的普通终端输入。
    ///
    /// 参数:
    /// - `event`: 待交给下一次输入框处理的事件
    ///
    /// 返回:
    /// - 无
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::cli) fn queue_input_event(&mut self, event: Event) {
        self.pending_input_events.push_back(event);
    }

    /// 读取模型运行期间保存的最早终端输入。
    ///
    /// 返回:
    /// - 下一条待处理事件
    pub(in crate::cli) fn pop_input_event(&mut self) -> Option<Event> {
        self.pending_input_events.pop_front()
    }

    /// 切换最近命令输出或思考段落的展开状态并重绘 TUI。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 是否找到可切换的命令输出
    pub(in crate::cli) fn toggle_command_output(&mut self) -> Result<bool> {
        // 1. 测试：只切换 transcript 状态，避免 replay 触碰真实终端
        if cfg!(test) {
            return Ok(self.transcript.toggle_latest_command_output());
        }
        // 2. 交互终端：备用屏 pager 展示全部折叠块，左右切换
        if std::io::stdout().is_terminal() && std::io::stdin().is_terminal() {
            let blocks = self.transcript.expandable_blocks();
            if blocks.is_empty() {
                return Ok(false);
            }
            let start = blocks.len().saturating_sub(1);
            super::super::repl_pager::open_blocks_pager(&blocks, start)?;
            // 备用屏返回后强制重同步 viewport 与 composer，避免输入框错位
            self.resync_after_overlay()?;
            return Ok(true);
        }
        // 3. 非交互：内联展开/折叠
        if !self.transcript.toggle_latest_command_output() {
            return Ok(false);
        }
        self.replay(false)?;
        self.redraw_stream_composer()?;
        Ok(true)
    }

    /// 切换当前实时思考块的展开状态并立即重绘。
    ///
    /// 返回:
    /// - 找到实时思考块时返回 true
    pub(in crate::cli) fn toggle_live_reasoning(&mut self) -> Result<bool> {
        // 流式期间只切 live 思考；定稿思考 / diff 走空闲时的备用屏 pager
        if !self.transcript.toggle_live_reasoning() {
            return Ok(false);
        }
        self.sync_transcript(true)?;
        self.redraw_stream_composer()?;
        Ok(true)
    }
}

/// 在流式事件循环 tick 中采样尺寸并执行到期 reflow 与 live 刷新。
///
/// 参数:
/// - `runtime`: 当前 REPL 运行期
///
/// 返回:
/// - 处理是否成功
pub(crate) fn process_stream_tick(runtime: &mut ReplRuntime) -> Result<()> {
    runtime.observe_terminal_size(true)?;
    runtime.maybe_reflow_due(true)?;
    runtime.tick_live()?;
    runtime.clamp_queue_panel();
    if runtime.tick_subagents()? {
        // 子 agent 状态变化会改变底部 agent 面板的计数与状态行
        runtime.redraw_stream_composer()?;
    }
    Ok(())
}

/// 处理模型运行期间的非阻塞终端事件。
///
/// Agent 工作时允许编辑底部输入框：Tab 入队，Shift+Tab 切换模式，Ctrl+C 中断。
/// 只读命令立即执行，打断类命令拒绝并提示，`/exit` 立即退出。
///
/// 参数:
/// - `runtime`: 当前 REPL 运行期
/// - `ctx`: 轮次开始时抓取的命令上下文
///
/// 返回:
/// - 键盘请求中断、退出或继续
pub(crate) fn process_stream_input(
    runtime: &mut ReplRuntime,
    ctx: &StreamCommandContext,
) -> Result<StreamInputAction> {
    while event::poll(Duration::ZERO)? {
        let input = event::read()?;
        match input {
            Event::Resize(cols, rows) => {
                // 流式期间只登记（streaming 语义），由 25ms tick 的 debounce
                // 到期重放统一重锚；立即重绘会用旧 origin 画错位置
                runtime.observe_stream_resize(cols, rows);
            }
            Event::Paste(text) => {
                let text = strip_control_sequences(&text);
                let draft = runtime.stream_draft_mut();
                draft.windows_paste.reset();
                // Windows 终端把 Ctrl+V 转成括号粘贴的文本事件，图片不会以文本
                // 形式到达；剪贴板里有图时先按图片插入，否则按普通文本粘贴
                #[cfg(windows)]
                if draft
                    .clipboard
                    .paste_image_if_any(&mut draft.text, &mut draft.cursor)
                {
                    draft.is_pasted = true;
                    draft.slash_selection = 0;
                    runtime.redraw_stream_composer()?;
                    continue;
                }
                draft
                    .clipboard
                    .paste_text_into_input(&mut draft.text, &mut draft.cursor, text);
                draft.is_pasted = true;
                draft.slash_selection = 0;
                runtime.redraw_stream_composer()?;
            }
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                let replay_key = windows_paste_key(key.code, key.modifiers);
                if replay_key.is_some_and(|candidate| {
                    runtime
                        .stream_draft_mut()
                        .windows_paste
                        .consume_key(candidate)
                }) {
                    continue;
                }
                if super::super::repl_transcript_pager::is_jump_to_output_bottom_key(
                    key.code,
                    key.modifiers,
                ) {
                    runtime.jump_to_output_bottom(true)?;
                    continue;
                }
                let ctrl_o = matches!(key.code, KeyCode::Char('o'))
                    && key.modifiers.contains(KeyModifiers::CONTROL);
                if !(matches!(key.code, KeyCode::Char('y') | KeyCode::Char('Y'))
                    && key.modifiers.contains(KeyModifiers::CONTROL))
                {
                    runtime.pending_clear_queue = false;
                }
                if ctrl_o && runtime.toggle_live_reasoning()? {
                    continue;
                }
                if matches!(key.code, KeyCode::Char('t'))
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                    && runtime.toggle_todo_panel_compact()
                {
                    runtime.redraw_stream_composer()?;
                    continue;
                }
                // Ctrl+Z 撤回队尾、Ctrl+Y 清空队列：排错入队的消息不必等本轮结束
                if matches!(key.code, KeyCode::Char('z'))
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    if runtime.undo_last_queued()?.is_none() {
                        runtime.record_meta(
                            crate::i18n::text(
                                "nothing to undo: the queue is empty or the input box is not empty",
                                "无可撤回项：队列为空，或输入框还有内容",
                            )
                            .to_string(),
                        )?;
                    }
                    continue;
                }
                if matches!(key.code, KeyCode::Char('y'))
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    let count = runtime.queue_len() + runtime.queued_control_commands().len();
                    if count == 0 {
                        runtime.pending_clear_queue = false;
                        continue;
                    }
                    if !runtime.pending_clear_queue {
                        runtime.pending_clear_queue = true;
                        runtime.record_meta(if crate::i18n::is_zh() {
                            format!("再按一次 Ctrl+Y 清空 {count} 条排队项")
                        } else {
                            format!("press Ctrl+Y again to clear {count} queued items")
                        })?;
                        continue;
                    }
                    runtime.pending_clear_queue = false;
                    let cleared = runtime.clear_queued()?;
                    if cleared > 0 {
                        runtime.record_meta(if crate::i18n::is_zh() {
                            format!("已清空 {cleared} 条排队项")
                        } else {
                            format!("cleared {cleared} queued items")
                        })?;
                    }
                    continue;
                }
                if ctrl_o {
                    // 轮次内没有可折叠的思考块时给 Ctrl+O 自己的提示。
                    // 不能复用下面 PageUp 的 pager 文案：用户按的是 Ctrl+O，
                    // 看到一条关于会话浏览面板的说明只会以为这个键坏了
                    runtime.record_meta(
                        crate::i18n::text(
                            "Ctrl+O: no collapsible reasoning block in this turn",
                            "Ctrl+O：本轮没有可折叠的思考块",
                        )
                        .to_string(),
                    )?;
                    continue;
                }
                if matches!(key.code, KeyCode::PageUp) {
                    // 1. 流式期间不打开阻塞式浏览面板：pager 会同步占住事件循环，
                    //    模型流无人读取、工具子进程管道写满后挂起
                    runtime.record_meta(
                        crate::i18n::text(
                            "transcript pager is available after this turn finishes",
                            "会话浏览面板需等本轮结束后再打开",
                        )
                        .to_string(),
                    )?;
                    continue;
                }
                if matches!(key.code, KeyCode::Char('c'))
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    // 2. Ctrl+C 中断当前轮
                    return Ok(StreamInputAction::Interrupt);
                }
                // 3. 用户消息队列管理（Ctrl+↑ 进入，↓ 离开末项回到输入框）
                if runtime.handle_queue_panel_key(key.code, key.modifiers)? {
                    continue;
                }
                if matches!(key.code, KeyCode::Enter) && key.modifiers.is_empty() {
                    let paste = {
                        let draft = runtime.stream_draft_mut();
                        let input_before_cursor: String =
                            draft.text.chars().take(draft.cursor).collect();
                        draft.windows_paste.begin_from_system_clipboard(
                            &input_before_cursor,
                            std::time::Instant::now(),
                        )
                    };
                    if let Some(paste) = paste {
                        let draft = runtime.stream_draft_mut();
                        draft.clipboard.replace_recent_text_with_paste(
                            &mut draft.text,
                            &mut draft.cursor,
                            paste.prefix_chars,
                            paste.text,
                        );
                        draft.is_pasted = true;
                        draft.slash_selection = 0;
                        runtime.redraw_stream_composer()?;
                        continue;
                    }
                }
                // 4. 底部 agent 面板优先消费按键（空输入 ↓ 展开、↑ 收回到输入框）
                if (runtime.agent_panel_active() || runtime.stream_draft().text.is_empty())
                    && runtime.handle_agent_panel_key(key.code)?
                {
                    continue;
                }
                // 5. 其他键写入运行中输入框
                let action = handle_stream_key(runtime, ctx, key.code, key.modifiers)?;
                if action != StreamInputAction::Continue {
                    return Ok(action);
                }
            }
            Event::Key(_) => {}
            _ => {}
        }
    }
    Ok(StreamInputAction::Continue)
}

/// 将单个按键应用到运行中 composer 草稿。
///
/// 参数:
/// - `runtime`: REPL 运行期
/// - `ctx`: 轮次开始时抓取的命令上下文
/// - `code`: 键码
/// - `modifiers`: 修饰键
///
/// 返回:
/// - 按键请求中断、退出或继续
fn handle_stream_key(
    runtime: &mut ReplRuntime,
    ctx: &StreamCommandContext,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> Result<StreamInputAction> {
    match code {
        KeyCode::BackTab => {
            // 部分终端把 Shift+Tab 发成 BackTab：立即生效
            let current = runtime.stream_mode(AgentMode::Yolo);
            let next = cycle_mode(current);
            runtime.stream_draft_mut().mode = Some(next);
            let _ = runtime.apply_stream_mode_live(AgentMode::Yolo);
            runtime.redraw_stream_composer()?;
        }
        KeyCode::Tab if modifiers.contains(KeyModifiers::SHIFT) => {
            // Shift+Tab：立即切换权限模式（当前轮生效）
            let current = runtime.stream_mode(AgentMode::Yolo);
            let next = cycle_mode(current);
            runtime.stream_draft_mut().mode = Some(next);
            let _ = runtime.apply_stream_mode_live(AgentMode::Yolo);
            runtime.redraw_stream_composer()?;
        }
        KeyCode::Tab => {
            // Tab：先补全 #/@ 引用，再补全斜杠命令，否则入队
            if complete_stream_mention(runtime)? {
                return Ok(StreamInputAction::Continue);
            }
            if runtime.stream_draft().text.starts_with('/') {
                // 运行期间不补全置灰命令：Tab 出一条执行不了的命令没有意义
                let completed = crate::cli::repl_commands::complete_repl_command(
                    &runtime.stream_draft().text,
                    true,
                );
                if let Some(completed) = completed {
                    let draft = runtime.stream_draft_mut();
                    draft.text = completed.to_string();
                    draft.cursor = draft.text.chars().count();
                    draft.slash_selection = 0;
                    runtime.redraw_stream_composer()?;
                    return Ok(StreamInputAction::Continue);
                }
            }
            if runtime.stream_draft().text.trim().is_empty() {
                runtime.record_meta(
                    crate::i18n::text("type a message before queuing", "输入内容后再排队")
                        .to_string(),
                )?;
                return Ok(StreamInputAction::Continue);
            }
            // 与 Enter 同一套分发：置灰命令拒绝留在输入框，避免本轮结束后悄悄执行
            return dispatch_stream_command(runtime, ctx);
        }
        KeyCode::Up => {
            // 引用或斜杠面板可见时上下键移动选中项
            let draft = runtime.stream_draft();
            let mentions =
                crate::cli::repl_mentions::find_mention_trigger(&draft.text, draft.cursor)
                    .map(|trigger| {
                        crate::cli::repl_mentions::mention_suggestions(
                            &trigger,
                            runtime.mention_skills(),
                        )
                    })
                    .unwrap_or_default();
            if !mentions.is_empty() {
                let draft = runtime.stream_draft_mut();
                draft.slash_selection = (draft.slash_selection % mentions.len())
                    .checked_sub(1)
                    .unwrap_or(mentions.len().saturating_sub(1));
                runtime.redraw_stream_composer()?;
                return Ok(StreamInputAction::Continue);
            }
            let suggestions = crate::cli::repl_commands::visible_repl_command_suggestions(
                &runtime.stream_draft().text,
                true,
            );
            if !suggestions.is_empty() {
                let draft = runtime.stream_draft_mut();
                draft.slash_selection = (draft.slash_selection % suggestions.len())
                    .checked_sub(1)
                    .unwrap_or(suggestions.len().saturating_sub(1));
                runtime.redraw_stream_composer()?;
            }
        }
        KeyCode::Down => {
            let draft = runtime.stream_draft();
            let mentions =
                crate::cli::repl_mentions::find_mention_trigger(&draft.text, draft.cursor)
                    .map(|trigger| {
                        crate::cli::repl_mentions::mention_suggestions(
                            &trigger,
                            runtime.mention_skills(),
                        )
                    })
                    .unwrap_or_default();
            if !mentions.is_empty() {
                let draft = runtime.stream_draft_mut();
                draft.slash_selection = (draft.slash_selection + 1) % mentions.len();
                runtime.redraw_stream_composer()?;
                return Ok(StreamInputAction::Continue);
            }
            let suggestions = crate::cli::repl_commands::visible_repl_command_suggestions(
                &runtime.stream_draft().text,
                true,
            );
            if !suggestions.is_empty() {
                let draft = runtime.stream_draft_mut();
                draft.slash_selection = (draft.slash_selection + 1) % suggestions.len();
                runtime.redraw_stream_composer()?;
            }
        }
        KeyCode::Enter => {
            if modifiers.contains(KeyModifiers::SHIFT) {
                let draft = runtime.stream_draft_mut();
                insert_char(&mut draft.text, &mut draft.cursor, '\n');
                draft.slash_selection = 0;
                draft.is_pasted = false;
                runtime.redraw_stream_composer()?;
                return Ok(StreamInputAction::Continue);
            }
            if complete_stream_mention(runtime)? {
                return Ok(StreamInputAction::Continue);
            }
            // Enter：面板可见时先落选中命令，否则 /model 会被当普通文本发出
            let suggestions = crate::cli::repl_commands::visible_repl_command_suggestions(
                &runtime.stream_draft().text,
                true,
            );
            if !suggestions.is_empty() {
                let draft = runtime.stream_draft_mut();
                let selected = suggestions[draft
                    .slash_selection
                    .min(suggestions.len().saturating_sub(1))];
                draft.text = selected.command.to_string();
                draft.cursor = draft.text.chars().count();
                draft.slash_selection = 0;
            }
            return dispatch_stream_command(runtime, ctx);
        }
        KeyCode::Backspace => {
            let draft = runtime.stream_draft_mut();
            if !draft
                .clipboard
                .remove_block_before_cursor(&mut draft.text, &mut draft.cursor)
                && draft.cursor > 0
            {
                remove_char_before(&mut draft.text, &mut draft.cursor);
            }
            draft.slash_selection = 0;
            draft.is_pasted = false;
            runtime.redraw_stream_composer()?;
        }
        KeyCode::Delete => {
            let draft = runtime.stream_draft_mut();
            if !draft
                .clipboard
                .remove_block_at_cursor(&mut draft.text, draft.cursor)
                && draft.cursor < draft.text.chars().count()
            {
                remove_char_at(&mut draft.text, draft.cursor);
            }
            draft.slash_selection = 0;
            draft.is_pasted = false;
            runtime.redraw_stream_composer()?;
        }
        KeyCode::Left => {
            // 剪贴板占位块整体跳过，保持与删除一致的原子性
            let draft = runtime.stream_draft_mut();
            draft.cursor = draft.clipboard.cursor_left(&draft.text, draft.cursor);
            runtime.redraw_stream_composer()?;
        }
        KeyCode::Right => {
            let draft = runtime.stream_draft_mut();
            draft.cursor = draft.clipboard.cursor_right(&draft.text, draft.cursor);
            runtime.redraw_stream_composer()?;
        }
        KeyCode::Home => {
            runtime.stream_draft_mut().cursor = 0;
            runtime.redraw_stream_composer()?;
        }
        KeyCode::End => {
            let draft = runtime.stream_draft_mut();
            draft.cursor = draft.text.chars().count();
            runtime.redraw_stream_composer()?;
        }
        KeyCode::Char('v') if modifiers.contains(KeyModifiers::CONTROL) => {
            let draft = runtime.stream_draft_mut();
            draft.windows_paste.reset();
            draft.is_pasted = draft
                .clipboard
                .paste_into_input(&mut draft.text, &mut draft.cursor)?;
            draft.slash_selection = 0;
            runtime.redraw_stream_composer()?;
        }
        KeyCode::Char(ch)
            if !modifiers.contains(KeyModifiers::CONTROL)
                && !modifiers.contains(KeyModifiers::ALT)
                && !is_control_char(ch) =>
        {
            let draft = runtime.stream_draft_mut();
            draft
                .windows_paste
                .record_char(ch, std::time::Instant::now());
            insert_char(&mut draft.text, &mut draft.cursor, ch);
            draft.slash_selection = 0;
            draft.is_pasted = false;
            runtime.redraw_stream_composer()?;
        }
        _ => {}
    }
    Ok(StreamInputAction::Continue)
}

/// 按执行策略分发运行期间提交的命令。
///
/// 只读命令立刻执行，打断类命令拒绝并保留草稿，退出命令返回退出信号，
/// 其余按普通消息入队等本轮结束。
///
/// 参数:
/// - `runtime`: REPL 运行期
/// - `ctx`: 轮次开始时抓取的命令上下文
///
/// 返回:
/// - 请求退出、中断或继续
fn dispatch_stream_command(
    runtime: &mut ReplRuntime,
    ctx: &StreamCommandContext,
) -> Result<StreamInputAction> {
    let text = runtime.stream_draft().text.trim().to_string();
    if text.is_empty() {
        return Ok(StreamInputAction::Continue);
    }
    // shell 命令保持入队：它本就是"本轮结束后跑"的语义，且沉底面板会
    // 常驻列出待执行项，用户可预期；拒绝会让 `!ls` 这类操作彻底不可用
    if text.starts_with('!') {
        return enqueue_stream_draft(runtime);
    }
    match stream_command_policy(&text) {
        StreamCommandPolicy::Exit => Ok(StreamInputAction::Exit),
        StreamCommandPolicy::Immediate => {
            super::stream_commands::run_immediate_stream_command(runtime, ctx, &text)?;
            let mode = runtime.stream_draft().mode;
            *runtime.stream_draft_mut() = StreamComposerDraft {
                mode,
                ..StreamComposerDraft::default()
            };
            runtime.redraw_stream_composer()?;
            Ok(StreamInputAction::Continue)
        }
        // 置灰命令不放进队列：静默排队正是用户看不到命令何时执行的根因，
        // 明确拒绝并把原文留在输入框里，本轮结束后直接回车即可
        StreamCommandPolicy::Disabled => {
            let hint = stream_command_disabled_hint(&text);
            runtime.record_meta(hint)?;
            runtime.redraw_stream_composer()?;
            Ok(StreamInputAction::Continue)
        }
        StreamCommandPolicy::NotCommand => enqueue_stream_draft(runtime),
    }
}

/// 把当前流式草稿按普通消息入队。
///
/// 参数:
/// - `runtime`: REPL 运行期
///
/// 返回:
/// - 入队结果包装为继续运行
fn enqueue_stream_draft(runtime: &mut ReplRuntime) -> Result<StreamInputAction> {
    let mode = runtime.stream_mode(AgentMode::Yolo);
    let _ = runtime.enqueue_stream_draft(mode)?;
    Ok(StreamInputAction::Continue)
}

/// 确认流式草稿中的 `#` / `@` 引用。
///
/// 参数:
/// - `runtime`: REPL 运行期
///
/// 返回:
/// - 已补全时为真
fn complete_stream_mention(runtime: &mut ReplRuntime) -> Result<bool> {
    let draft = runtime.stream_draft();
    let Some(trigger) = crate::cli::repl_mentions::find_mention_trigger(&draft.text, draft.cursor)
    else {
        return Ok(false);
    };
    let suggestions =
        crate::cli::repl_mentions::mention_suggestions(&trigger, runtime.mention_skills());
    let Some(item) = suggestions
        .get(
            draft
                .slash_selection
                .min(suggestions.len().saturating_sub(1)),
        )
        .cloned()
    else {
        return Ok(false);
    };
    let draft = runtime.stream_draft_mut();
    let (next, cursor) = crate::cli::repl_mentions::apply_mention(&draft.text, &trigger, &item);
    draft.text = next;
    draft.cursor = cursor;
    draft.slash_selection = 0;
    draft.is_pasted = false;
    runtime.redraw_stream_composer()?;
    Ok(true)
}

/// 把终端按键转换为 Windows 粘贴回放可比较的字符。
///
/// 参数:
/// - `code`: 终端键码
/// - `modifiers`: 终端修饰键
///
/// 返回:
/// - 可参与剪贴板回放匹配的按键
fn windows_paste_key(code: KeyCode, modifiers: KeyModifiers) -> Option<WindowsPasteKey> {
    match code {
        KeyCode::Char(ch)
            if !modifiers.contains(KeyModifiers::CONTROL)
                && !modifiers.contains(KeyModifiers::ALT) =>
        {
            Some(WindowsPasteKey::Char(ch))
        }
        KeyCode::Enter if modifiers.is_empty() => Some(WindowsPasteKey::Enter),
        KeyCode::Tab if modifiers.is_empty() => Some(WindowsPasteKey::Tab),
        _ => None,
    }
}

/// 循环切换 Agent 模式。
fn cycle_mode(mode: AgentMode) -> AgentMode {
    match mode {
        AgentMode::Yolo => AgentMode::Audited,
        AgentMode::Audited => AgentMode::AutoAudit,
        AgentMode::AutoAudit => AgentMode::Plan,
        AgentMode::Plan => AgentMode::Yolo,
    }
}

/// 在光标处插入字符。
fn insert_char(input: &mut String, cursor: &mut usize, ch: char) {
    let byte = input
        .char_indices()
        .nth(*cursor)
        .map(|(index, _)| index)
        .unwrap_or(input.len());
    input.insert(byte, ch);
    *cursor += 1;
}

/// 删除光标前一个字符。
fn remove_char_before(input: &mut String, cursor: &mut usize) {
    if *cursor == 0 {
        return;
    }
    let start = input
        .char_indices()
        .nth(*cursor - 1)
        .map(|(index, _)| index)
        .unwrap_or(0);
    let end = input
        .char_indices()
        .nth(*cursor)
        .map(|(index, _)| index)
        .unwrap_or(input.len());
    input.replace_range(start..end, "");
    *cursor -= 1;
}

/// 删除光标处字符。
fn remove_char_at(input: &mut String, cursor: usize) {
    let Some((start, ch)) = input.char_indices().nth(cursor) else {
        return;
    };
    let end = start + ch.len_utf8();
    input.replace_range(start..end, "");
}

/// 判断是否为不应写入输入框的控制字符。
fn is_control_char(ch: char) -> bool {
    ch.is_control() && ch != '\n' && ch != '\t'
}

/// 去掉终端控制序列，避免粘贴污染输入框。
fn strip_control_sequences(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            // 跳过 CSI / 简单 ESC 序列
            if chars.peek() == Some(&'[') {
                chars.next();
                for next in chars.by_ref() {
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        if ch == '\r' {
            continue;
        }
        out.push(ch);
    }
    out
}
