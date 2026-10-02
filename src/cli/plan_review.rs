use crate::question::{PendingQuestion, QuestionResponse};
use anyhow::Result;

/// 【计划模式】【终端审阅】参数为待审批请求与是否使用临时屏幕，返回用户决定；正文先在可滚动 Markdown 预览中完整展示。
pub(super) fn ask(pending: &PendingQuestion, overlay: bool) -> Result<QuestionResponse> {
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
        super::question_screen::ask(&pending.request)
    } else {
        crate::question_tui::ask(&pending.request)
    }
}
