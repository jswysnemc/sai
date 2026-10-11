use crate::question::{QuestionRequest, QuestionResponse};
use crate::question_tui::OverlayPlacement;
use anyhow::Result;

/// 【终端提问】【底部覆盖】参数为问题请求与覆盖区域，返回用户决定；不进备用屏、不清整屏。
pub(super) fn ask(
    request: &QuestionRequest,
    placement: OverlayPlacement,
) -> Result<QuestionResponse> {
    if !crate::question_tui::available(false) {
        return Ok(QuestionResponse::Unavailable(
            "Interactive terminal unavailable".into(),
        ));
    }
    crate::question_tui::ask_overlay(request, placement)
}
