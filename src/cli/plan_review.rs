use crate::i18n::text as t;
use crate::question::{PendingQuestion, QuestionResponse};
use crate::question_tui::OverlayPlacement;
use anyhow::Result;

/// 【计划模式】【终端审阅】先在分页器预览完整计划文本，关闭后弹出底部或内联提问卡片收集审批决定。
///
/// 参数:
/// - `pending`: 待审批的计划与提问请求
/// - `overlay`: 是否以底部覆盖层形态弹出提问卡片
/// - `placement`: 可选预设覆盖区域
///
/// 返回:
/// - 用户的审批回答或终端不可用说明
pub(super) fn ask(
    pending: &PendingQuestion,
    overlay: bool,
    placement: Option<OverlayPlacement>,
) -> Result<QuestionResponse> {
    if let Some(plan) = &pending.plan {
        if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
            return Ok(QuestionResponse::Unavailable(
                t(
                    "Plan review requires an interactive terminal",
                    "计划审阅需要交互式终端",
                )
                .into(),
            ));
        }
        // 1. 【计划模式】【全屏预览】先打开 Markdown 分页预览，关闭后进入审批选项
        let block = crate::render::transcript::ExpandableBlock {
            title: t(
                "Plan review · close preview to approve or request changes",
                "计划审阅 · 关闭预览后可批准或提出修改意见",
            )
            .into(),
            body: plan.clone(),
            kind: crate::render::transcript::ExpandableBlockKind::Markdown,
        };
        super::repl_pager::open_blocks_pager(0, 1, move |_, width| {
            crate::render::transcript::PagerView::from_lines(super::repl_pager::render_block_lines(
                &block, width,
            ))
        })?;
    }
    // 2. 【计划模式】【提问收集】按当前屏幕形态选择覆盖层或主屏内联卡片
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
