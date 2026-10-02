use crate::question::{QuestionRequest, QuestionResponse};
use anyhow::Result;
use crossterm::{
    cursor::MoveTo,
    execute,
    style::Print,
    terminal::{Clear, ClearType},
};
use std::io;

/// 【终端提问】【临时屏幕】隔离卡片预留空间和完成摘要，保证主屏工具标题仍可原地更新。
struct QuestionScreen;

impl QuestionScreen {
    /// 【终端提问】【进入屏幕】无参数，返回自动恢复主屏的守卫。
    fn enter() -> Result<Self> {
        let mut stdout = io::stdout();
        super::alternate_screen::enter_alternate_screen(&mut stdout)?;
        let screen = Self;
        execute!(
            stdout,
            Print("\x1b[0m"),
            Clear(ClearType::All),
            MoveTo(0, 0)
        )?;
        Ok(screen)
    }
}

impl Drop for QuestionScreen {
    /// 【终端提问】【恢复屏幕】无参数，正常返回和异常退出都恢复主屏与键盘协议。
    fn drop(&mut self) {
        let _ = super::alternate_screen::leave_alternate_screen(&mut io::stdout());
    }
}

/// 【终端提问】【隔离交互】参数为问题请求，返回用户决定；主屏不保留临时卡片或重复摘要。
pub(super) fn ask(request: &QuestionRequest) -> Result<QuestionResponse> {
    if !crate::question_tui::available(false) {
        return Ok(QuestionResponse::Unavailable(
            "Interactive terminal unavailable".into(),
        ));
    }
    let _screen = QuestionScreen::enter()?;
    crate::question_tui::ask(request)
}
