use crate::question::{PendingQuestion, QuestionResponse};
use crate::question_tui::OverlayPlacement;
use anyhow::Result;

/// 【计划模式】【终端审阅】参数为待审批请求、是否底部覆盖与覆盖区域，返回用户决定。
pub(super) fn ask(
    pending: &PendingQuestion,
    overlay: bool,
    placement: Option<OverlayPlacement>,
) -> Result<QuestionResponse> {
    if let Some(plan) = &pending.plan {
        if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
            return Ok(QuestionResponse::Unavailable(
                "Plan review requires an interactive terminal".into(),
            ));
        }
        let block = crate::render::transcript::ExpandableBlock {
            title: "Plan review · close preview to approve or request changes".into(),
            body: plan.clone(),
            kind: crate::render::transcript::ExpandableBlockKind::Markdown,
        };
        super::repl_pager::open_blocks_pager(0, 1, move |_, width| {
            crate::render::transcript::PagerView::from_lines(super::repl_pager::render_block_lines(
                &block, width,
            ))
        })?;
    }
    if overlay {
        let placement = placement.unwrap_or_else(|| {
            let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
            OverlayPlacement::bottom(cols, rows)
        });
        super::question_screen::ask(&pending.request, placement)
    } else {
        crate::question_tui::ask(&pending.request)
    }
}
