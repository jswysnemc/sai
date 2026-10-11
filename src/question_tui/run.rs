use super::render::draw;
use super::session::QuestionSession;
use super::state::{handle_editing_key, submitted_answers, QuestionState};
use super::text::insert_text;
use crate::question::{QuestionRequest, QuestionResponse};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use std::time::{Duration, Instant};

const CANCEL_CONFIRM_WINDOW: Duration = Duration::from_secs(2);

/// 【终端提问】【交互循环】参数为问题集合与终端会话，返回用户决定。
pub(super) fn run(
    request: &QuestionRequest,
    session: &mut QuestionSession,
) -> Result<QuestionResponse> {
    let mut state = QuestionState::new(request);
    loop {
        if state
            .cancel_armed_until
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            state.cancel_armed_until = None;
        }
        draw(session, request, &mut state)?;

        if !event::poll(Duration::from_millis(100))? {
            continue;
        }
        let event = event::read()?;
        match event {
            Event::Resize(cols, rows) => {
                let (cols, rows) = crate::platform::windows_console::viewport_resize(cols, rows);
                session.resize_to_terminal(cols, rows);
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
