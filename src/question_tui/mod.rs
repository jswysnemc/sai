mod card;
mod components;
mod navigation;
mod render;
mod session;
mod state;
mod summary;
mod symbols;
mod text;
mod view;

use self::render::draw;
use self::session::QuestionSession;
use self::state::{handle_editing_key, submitted_answers, QuestionState};
use self::text::{insert_text, remove_at_cursor, remove_before_cursor, reserve_space};
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
const BAR: &str = " ";

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
            Event::Resize(cols, rows) => {
                let (_, rows) = crate::platform::windows_console::viewport_resize(cols, rows);
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
                        KeyCode::Up => {
                            state.scroll_starts[state.tab] =
                                state.scroll_starts[state.tab].saturating_sub(1)
                        }
                        KeyCode::Down => {
                            state.scroll_starts[state.tab] =
                                state.scroll_starts[state.tab].saturating_add(1)
                        }
                        KeyCode::Left | KeyCode::BackTab | KeyCode::Char('h') => {
                            state.previous_tab(request)
                        }
                        KeyCode::Right | KeyCode::Tab | KeyCode::Char('l') => {
                            state.next_tab(request)
                        }
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
                    KeyCode::Char(number)
                        if number.is_ascii_digit()
                            && number != '0'
                            && (number.to_digit(10).unwrap_or(0) as usize)
                                <= question.options.len() + usize::from(question.custom)
                            && !key.modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        state
                            .activate_number(request, number.to_digit(10).unwrap_or(0) as usize)?;
                        if !request.needs_review() {
                            if let Some(answers) = submitted_answers(request, &state)? {
                                session.finish_answered(request, &answers)?;
                                return Ok(QuestionResponse::Answered(answers));
                            }
                        }
                    }
                    KeyCode::Left | KeyCode::Char('h') => state.previous_tab(request),
                    KeyCode::Right | KeyCode::Char('l') => state.next_tab(request),
                    KeyCode::Up | KeyCode::Char('k') => state.previous_option(question),
                    KeyCode::Down | KeyCode::Char('j') => state.next_option(question),
                    KeyCode::Char(' ') => {
                        state.toggle_current(request)?;
                    }
                    KeyCode::Tab => state.next_tab(request),
                    KeyCode::BackTab => state.previous_tab(request),
                    KeyCode::Enter if question.multiple => {
                        state.continue_multiple(request)?;
                    }
                    KeyCode::Enter => {
                        state.confirm_single(request)?;
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

#[cfg(test)]
mod tests;

#[cfg(test)]
mod view_tests;

#[cfg(test)]
mod interaction_tests;
