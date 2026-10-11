mod card;
mod components;
mod navigation;
mod overlay;
mod render;
mod run;
mod session;
mod state;
mod summary;
mod symbols;
mod text;
mod view;

pub use overlay::OverlayPlacement;

use self::overlay::MAX_PANEL_LINES;
use self::run::run;
use self::session::QuestionSession;
use self::text::reserve_space;
use crate::i18n::text as t;
use crate::question::{QuestionRequest, QuestionResponse};
use anyhow::{bail, Result};
use crossterm::terminal;
use std::io::{self, IsTerminal};

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
    run(request, &mut session)
}

/// 【终端提问】【底部覆盖】参数为问题集合与覆盖区域，返回用户回答；不预留滚动行、不开关 raw mode。
pub fn ask_overlay(
    request: &QuestionRequest,
    placement: OverlayPlacement,
) -> Result<QuestionResponse> {
    request.validate()?;
    if !available(false) {
        bail!(t("interactive terminal is unavailable", "交互式终端不可用"));
    }
    let mut session = QuestionSession::start_overlay(placement)?;
    run(request, &mut session)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod view_tests;

#[cfg(test)]
mod interaction_tests;
