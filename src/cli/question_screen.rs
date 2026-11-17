use crate::i18n::text as t;
use crate::question::{QuestionRequest, QuestionResponse};
use crate::question_tui::OverlayPlacement;
use anyhow::Result;

/// 【终端提问】【底部覆盖】在不进新备用屏、不清整屏的情况下打开底部提问覆盖卡片。
///
/// 参数:
/// - `request`: 问题请求集合
/// - `placement`: 底部覆盖区域
///
/// 返回:
/// - 用户提交的回答或不可用说明
pub(super) fn ask(
    request: &QuestionRequest,
    placement: OverlayPlacement,
) -> Result<QuestionResponse> {
    if !crate::question_tui::available(false) {
        return Ok(QuestionResponse::Unavailable(
            t("Interactive terminal unavailable", "交互式终端不可用").into(),
        ));
    }
    crate::question_tui::ask_overlay(request, placement)
}
