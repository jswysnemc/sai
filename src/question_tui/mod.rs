mod render;
mod text;

use self::render::draw;
use self::text::{
    display_inline, insert_text, remove_at_cursor, remove_before_cursor, reserve_space,
    truncate_width,
};
use crate::i18n::text as t;
use crate::question::{
    validate_answers, QuestionAnswers, QuestionPrompt, QuestionRequest, QuestionResponse,
};
use anyhow::{bail, Result};
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers,
};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::{execute, queue};
use std::io::{self, IsTerminal, Write};
use std::time::{Duration, Instant};

const MAX_PANEL_LINES: u16 = 16;
const CANCEL_CONFIRM_WINDOW: Duration = Duration::from_secs(2);
const BAR: &str = "\x1b[1m\x1b[35m┃\x1b[0m";
const ANSWERED_BAR: &str = "\x1b[2m\x1b[90m┃\x1b[0m";

/// 判断当前标准输出和终端设备是否支持交互式提问。
///
/// # 参数
/// - `plain`: 是否强制使用纯文本模式
///
/// # 返回值
/// 可以使用交互式终端时返回 `true`
pub fn available(plain: bool) -> bool {
    if plain || !io::stdout().is_terminal() {
        return false;
    }
    #[cfg(unix)]
    {
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/tty")
            .is_ok()
    }
    #[cfg(not(unix))]
    {
        io::stdin().is_terminal()
    }
}

/// 在交互式终端中执行结构化提问。
///
/// # 参数
/// - `request`: 经过定义的结构化问题集合
///
/// # 返回值
/// 用户回答或取消结果；终端不可用或操作失败时返回错误
pub fn ask(request: &QuestionRequest) -> Result<QuestionResponse> {
    request.validate()?;
    if !available(false) {
        bail!(t("interactive terminal is unavailable", "交互式终端不可用"));
    }

    let panel_lines = terminal::size()
        .map(|(_, rows)| rows.saturating_sub(1).clamp(1, MAX_PANEL_LINES))
        .unwrap_or(12);
    reserve_space(panel_lines)?;
    let mut session = QuestionSession::start(panel_lines)?;
    let mut state = QuestionState::new(request);

    loop {
        if state
            .cancel_armed_until
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            state.cancel_armed_until = None;
        }
        draw(&mut session, request, &mut state)?;

        if !event::poll(Duration::from_millis(100))? {
            continue;
        }
        let event = event::read()?;
        match event {
            Event::Resize(_, rows) => {
                session.resize_to_terminal(rows);
                continue;
            }
            Event::Paste(text) if state.editing => {
                insert_text(&mut state.edit_buffer, &mut state.edit_cursor, &text);
            }
            Event::Key(key) => {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                if matches!(key.code, KeyCode::Char('c'))
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    session.finish_cancelled()?;
                    return Ok(QuestionResponse::Cancelled);
                }
                if state.editing {
                    if handle_editing_key(request, &mut state, key)? && !request.needs_review() {
                        if let Some(answers) = submitted_answers(request, &state)? {
                            session.finish_answered(request, &answers)?;
                            return Ok(QuestionResponse::Answered(answers));
                        }
                    }
                    continue;
                }

                if key.code == KeyCode::Esc {
                    if state
                        .cancel_armed_until
                        .is_some_and(|deadline| Instant::now() < deadline)
                    {
                        session.finish_cancelled()?;
                        return Ok(QuestionResponse::Cancelled);
                    }
                    state.cancel_armed_until = Some(Instant::now() + CANCEL_CONFIRM_WINDOW);
                    continue;
                }
                state.cancel_armed_until = None;

                if state.on_confirm(request) {
                    match key.code {
                        KeyCode::Left | KeyCode::Char('h') => state.previous_tab(request),
                        KeyCode::Right | KeyCode::Char('l') => state.next_tab(request),
                        KeyCode::Enter => {
                            if let Some(answers) = submitted_answers(request, &state)? {
                                session.finish_answered(request, &answers)?;
                                return Ok(QuestionResponse::Answered(answers));
                            }
                            state.go_to_first_unanswered(request);
                        }
                        _ => {}
                    }
                    continue;
                }

                let question = &request.questions[state.tab];
                match key.code {
                    KeyCode::Left | KeyCode::Char('h') => state.previous_tab(request),
                    KeyCode::Right | KeyCode::Char('l') => state.next_tab(request),
                    KeyCode::Up | KeyCode::Char('k') => state.previous_option(question),
                    KeyCode::Down | KeyCode::Char('j') => state.next_option(question),
                    KeyCode::Tab | KeyCode::Char(' ') if question.multiple => {
                        state.toggle_current(request)?;
                    }
                    KeyCode::Enter if question.multiple => {
                        state.activate_current(request)?;
                    }
                    KeyCode::Enter => {
                        state.activate_current(request)?;
                        if !request.needs_review() {
                            if let Some(answers) = submitted_answers(request, &state)? {
                                session.finish_answered(request, &answers)?;
                                return Ok(QuestionResponse::Answered(answers));
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

struct QuestionState {
    tab: usize,
    selected: Vec<usize>,
    scroll_starts: Vec<usize>,
    answers: QuestionAnswers,
    custom_answers: Vec<String>,
    editing: bool,
    edit_buffer: String,
    edit_cursor: usize,
    cancel_armed_until: Option<Instant>,
}

impl QuestionState {
    /// 根据提问请求创建初始交互状态。
    ///
    /// 参数:
    /// - `request`: 结构化提问请求
    ///
    /// 返回:
    /// - 尚未回答的初始状态
    fn new(request: &QuestionRequest) -> Self {
        Self {
            tab: 0,
            selected: vec![0; request.questions.len()],
            scroll_starts: vec![0; request.questions.len() + usize::from(request.needs_review())],
            answers: vec![Vec::new(); request.questions.len()],
            custom_answers: vec![String::new(); request.questions.len()],
            editing: false,
            edit_buffer: String::new(),
            edit_cursor: 0,
            cancel_armed_until: None,
        }
    }

    /// 判断当前标签是否为最终确认页。
    ///
    /// 参数:
    /// - `request`: 结构化提问请求
    ///
    /// 返回:
    /// - 当前位于确认页时返回 `true`
    fn on_confirm(&self, request: &QuestionRequest) -> bool {
        request.needs_review() && self.tab == request.questions.len()
    }

    /// 计算问题标签和确认标签的总数。
    ///
    /// 参数:
    /// - `request`: 结构化提问请求
    ///
    /// 返回:
    /// - 可切换标签总数
    fn tab_count(&self, request: &QuestionRequest) -> usize {
        request.questions.len() + usize::from(request.needs_review())
    }

    /// 切换到前一个问题或确认标签。
    ///
    /// 参数:
    /// - `request`: 结构化提问请求
    ///
    /// 返回:
    /// - 无
    fn previous_tab(&mut self, request: &QuestionRequest) {
        let count = self.tab_count(request);
        self.tab = (self.tab + count - 1) % count;
    }

    /// 切换到后一个问题或确认标签。
    ///
    /// 参数:
    /// - `request`: 结构化提问请求
    ///
    /// 返回:
    /// - 无
    fn next_tab(&mut self, request: &QuestionRequest) {
        self.tab = (self.tab + 1) % self.tab_count(request);
    }

    /// 在当前问题中选择前一个选项。
    ///
    /// 参数:
    /// - `question`: 当前问题
    ///
    /// 返回:
    /// - 无
    fn previous_option(&mut self, question: &QuestionPrompt) {
        let count = option_count(question);
        if count > 0 {
            let selected = &mut self.selected[self.tab];
            *selected = (*selected + count - 1) % count;
        }
    }

    /// 在当前问题中选择后一个选项。
    ///
    /// 参数:
    /// - `question`: 当前问题
    ///
    /// 返回:
    /// - 无
    fn next_option(&mut self, question: &QuestionPrompt) {
        let count = option_count(question);
        if count > 0 {
            self.selected[self.tab] = (self.selected[self.tab] + 1) % count;
        }
    }

    /// 激活当前选项，或进入自定义答案编辑状态。
    ///
    /// 参数:
    /// - `request`: 结构化提问请求
    ///
    /// 返回:
    /// - 操作成功时返回空结果
    fn activate_current(&mut self, request: &QuestionRequest) -> Result<()> {
        let question = &request.questions[self.tab];
        let selected = self.selected[self.tab];
        if selected == question.options.len() && question.custom {
            self.editing = true;
            self.edit_buffer = self.custom_answers[self.tab].clone();
            self.edit_cursor = self.edit_buffer.chars().count();
            return Ok(());
        }
        let Some(option) = question.options.get(selected) else {
            bail!(t(
                "selected question option is out of range",
                "选中的问题选项超出范围"
            ));
        };
        if question.multiple {
            toggle_answer(&mut self.answers[self.tab], &option.label);
        } else {
            self.answers[self.tab] = vec![option.label.clone()];
            self.advance_after_single(request);
        }
        Ok(())
    }

    /// 切换当前多选答案的选中状态。
    ///
    /// 参数:
    /// - `request`: 结构化提问请求
    ///
    /// 返回:
    /// - 操作成功时返回空结果
    fn toggle_current(&mut self, request: &QuestionRequest) -> Result<()> {
        let question = &request.questions[self.tab];
        let selected = self.selected[self.tab];
        if selected == question.options.len() && question.custom {
            let custom = self.custom_answers[self.tab].trim();
            if custom.is_empty() {
                return self.activate_current(request);
            }
            toggle_answer(&mut self.answers[self.tab], custom);
            return Ok(());
        }
        self.activate_current(request)
    }

    /// 单选完成后推进到下一个标签。
    ///
    /// 参数:
    /// - `request`: 结构化提问请求
    ///
    /// 返回:
    /// - 无
    fn advance_after_single(&mut self, request: &QuestionRequest) {
        if request.questions.len() == 1 && !request.needs_review() {
            return;
        }
        self.tab = (self.tab + 1).min(self.tab_count(request) - 1);
    }

    /// 将焦点移动到第一个未回答问题。
    ///
    /// 参数:
    /// - `request`: 结构化提问请求
    ///
    /// 返回:
    /// - 无
    fn go_to_first_unanswered(&mut self, request: &QuestionRequest) {
        if let Some(index) = self.answers.iter().position(Vec::is_empty) {
            self.tab = index.min(request.questions.len().saturating_sub(1));
        }
    }
}

fn handle_editing_key(
    request: &QuestionRequest,
    state: &mut QuestionState,
    key: KeyEvent,
) -> Result<bool> {
    match key.code {
        KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            insert_text(&mut state.edit_buffer, &mut state.edit_cursor, "\n");
        }
        KeyCode::Esc => {
            state.editing = false;
            state.edit_buffer.clear();
            state.edit_cursor = 0;
        }
        KeyCode::Enter => {
            let value = state.edit_buffer.trim().to_string();
            if value.is_empty() {
                let previous = std::mem::take(&mut state.custom_answers[state.tab]);
                state.answers[state.tab].retain(|answer| answer != &previous);
                state.editing = false;
                state.edit_buffer.clear();
                state.edit_cursor = 0;
                return Ok(false);
            }
            let question = &request.questions[state.tab];
            let previous = std::mem::replace(&mut state.custom_answers[state.tab], value.clone());
            if !previous.is_empty() {
                state.answers[state.tab].retain(|answer| answer != &previous);
            }
            if question.multiple {
                if !state.answers[state.tab].contains(&value) {
                    state.answers[state.tab].push(value);
                }
            } else {
                state.answers[state.tab] = vec![value];
            }
            state.editing = false;
            state.edit_buffer.clear();
            state.edit_cursor = 0;
            if !question.multiple {
                state.advance_after_single(request);
            }
            return Ok(true);
        }
        KeyCode::Left => state.edit_cursor = state.edit_cursor.saturating_sub(1),
        KeyCode::Right => {
            state.edit_cursor = (state.edit_cursor + 1).min(state.edit_buffer.chars().count())
        }
        KeyCode::Home => state.edit_cursor = 0,
        KeyCode::End => state.edit_cursor = state.edit_buffer.chars().count(),
        KeyCode::Backspace => remove_before_cursor(&mut state.edit_buffer, &mut state.edit_cursor),
        KeyCode::Delete => remove_at_cursor(&mut state.edit_buffer, state.edit_cursor),
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            insert_text(
                &mut state.edit_buffer,
                &mut state.edit_cursor,
                &ch.to_string(),
            );
        }
        _ => {}
    }
    Ok(false)
}

/// 校验当前回答是否达到提交条件。
///
/// 参数:
/// - `request`: 结构化提问请求
/// - `state`: 当前回答状态
///
/// 返回:
/// - 可以提交时返回答案，否则返回空
fn submitted_answers(
    request: &QuestionRequest,
    state: &QuestionState,
) -> Result<Option<QuestionAnswers>> {
    if state.editing || state.answers.iter().any(Vec::is_empty) {
        return Ok(None);
    }
    if request.needs_review() && !state.on_confirm(request) {
        return Ok(None);
    }
    validate_answers(request, &state.answers)?;
    Ok(Some(state.answers.clone()))
}

/// 计算问题的预设选项和自定义选项总数。
///
/// 参数:
/// - `question`: 当前问题
///
/// 返回:
/// - 可选择项总数
fn option_count(question: &QuestionPrompt) -> usize {
    question.options.len() + usize::from(question.custom)
}

/// 切换指定答案在多选结果中的存在状态。
///
/// 参数:
/// - `answers`: 当前多选答案
/// - `value`: 需要切换的答案
///
/// 返回:
/// - 无
fn toggle_answer(answers: &mut Vec<String>, value: &str) {
    if let Some(index) = answers.iter().position(|answer| answer == value) {
        answers.remove(index);
    } else {
        answers.push(value.to_string());
    }
}

struct QuestionSession {
    stdout: io::Stdout,
    anchor_y: u16,
    panel_lines: u16,
}

impl QuestionSession {
    /// 启动原始模式终端会话并记录面板锚点。
    ///
    /// 参数:
    /// - `panel_lines`: 面板占用行数
    ///
    /// 返回:
    /// - 初始化完成的终端会话
    fn start(panel_lines: u16) -> Result<Self> {
        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        if let Err(err) = execute!(stdout, EnableBracketedPaste, Hide) {
            let _ = execute!(stdout, DisableBracketedPaste, Show);
            let _ = terminal::disable_raw_mode();
            return Err(err.into());
        }
        let (_, cursor_y) =
            crossterm::cursor::position().unwrap_or((0, panel_lines.saturating_sub(1)));
        let anchor_y = cursor_y.saturating_sub(panel_lines.saturating_sub(1));
        Ok(Self {
            stdout,
            anchor_y,
            panel_lines,
        })
    }

    /// 清理交互面板并输出回答摘要。
    ///
    /// 参数:
    /// - `request`: 结构化提问请求
    /// - `answers`: 已提交答案
    ///
    /// 返回:
    /// - 输出成功时返回空结果
    fn finish_answered(
        &mut self,
        request: &QuestionRequest,
        answers: &QuestionAnswers,
    ) -> Result<()> {
        self.clear()?;
        let width = terminal::size().map(|(cols, _)| cols).unwrap_or(80) as usize;
        let content_width = width.saturating_sub(3).max(1);
        let keeps_blank_line = self.panel_lines > 1;
        let content_rows = self
            .panel_lines
            .saturating_sub(u16::from(keeps_blank_line))
            .max(1);
        let answer_capacity = content_rows.saturating_sub(1) as usize;
        let omitted = request.questions.len().saturating_sub(answer_capacity);
        let mut row = 0u16;
        let heading = if omitted == 0 {
            format!(
                "{} {} {}",
                t("Answered", "已回答"),
                request.questions.len(),
                t("questions", "个问题")
            )
        } else {
            format!(
                "{} {} {} · {} {}",
                t("Answered", "已回答"),
                request.questions.len(),
                t("questions", "个问题"),
                t("omitted", "省略"),
                omitted
            )
        };
        self.write_answered_line(row, &heading, content_width)?;
        row += 1;
        for (question, selected) in request.questions.iter().zip(answers).take(answer_capacity) {
            self.write_answered_line(
                row,
                &format!(
                    "{}: {}",
                    question.header,
                    display_inline(&selected.join(t(" / ", "、")))
                ),
                content_width,
            )?;
            row += 1;
        }
        if keeps_blank_line {
            queue!(
                self.stdout,
                MoveTo(0, self.anchor_y.saturating_add(row)),
                Clear(ClearType::CurrentLine),
                crossterm::style::Print("\r\n")
            )?;
        } else {
            queue!(
                self.stdout,
                MoveTo(0, self.anchor_y.saturating_add(row.saturating_sub(1))),
                crossterm::style::Print("\r\n")
            )?;
        }
        queue!(self.stdout, Clear(ClearType::CurrentLine), Show)?;
        self.stdout.flush()?;
        Ok(())
    }

    /// 清理交互面板并输出取消状态。
    ///
    /// 返回:
    /// - 输出成功时返回空结果
    fn finish_cancelled(&mut self) -> Result<()> {
        self.clear()?;
        queue!(
            self.stdout,
            MoveTo(0, self.anchor_y),
            crossterm::style::Print(format!(
                "{BAR} \x1b[2m{}\x1b[0m",
                t("Question cancelled", "已取消提问")
            )),
            MoveTo(0, self.anchor_y.saturating_add(1)),
            Clear(ClearType::CurrentLine),
            Show
        )?;
        self.stdout.flush()?;
        Ok(())
    }

    /// 在指定面板行输出一条回答摘要。
    ///
    /// 参数:
    /// - `row`: 相对面板行号
    /// - `text`: 摘要文本
    /// - `width`: 最大显示宽度
    ///
    /// 返回:
    /// - 输出成功时返回空结果
    fn write_answered_line(&mut self, row: u16, text: &str, width: usize) -> Result<()> {
        queue!(
            self.stdout,
            MoveTo(0, self.anchor_y.saturating_add(row)),
            Clear(ClearType::CurrentLine),
            crossterm::style::Print(ANSWERED_BAR),
            crossterm::style::Print(" \x1b[2m\x1b[90m"),
            crossterm::style::Print(truncate_width(text, width)),
            crossterm::style::Print("\x1b[0m")
        )?;
        Ok(())
    }

    /// 清空当前面板占用的全部终端行。
    ///
    /// 返回:
    /// - 清理成功时返回空结果
    fn clear(&mut self) -> Result<()> {
        for row in 0..self.panel_lines {
            queue!(
                self.stdout,
                MoveTo(0, self.anchor_y.saturating_add(row)),
                Clear(ClearType::CurrentLine)
            )?;
        }
        Ok(())
    }

    /// 根据终端新高度调整面板尺寸和锚点。
    ///
    /// 参数:
    /// - `rows`: 终端总行数
    ///
    /// 返回:
    /// - 无
    fn resize_to_terminal(&mut self, rows: u16) {
        self.panel_lines = rows.saturating_sub(1).clamp(1, MAX_PANEL_LINES);
        self.anchor_y = self.anchor_y.min(rows.saturating_sub(self.panel_lines));
    }
}

impl Drop for QuestionSession {
    fn drop(&mut self) {
        let _ = execute!(self.stdout, DisableBracketedPaste, Show);
        let _ = terminal::disable_raw_mode();
    }
}

#[cfg(test)]
mod tests;
