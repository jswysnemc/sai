use super::super::fullscreen_parts::ParagraphKey;
use super::node::RenderedNode;
use std::cell::RefCell;
use std::collections::HashSet;

pub(in crate::render::transcript) const DETAIL_WINDOW_ROWS: usize = 10;

/// 【终端】【查看保护】在重绘期间冻结自动折叠并固定用户正在查看的步骤。
#[derive(Clone, Default)]
struct InspectionContext {
    freeze_fold: bool,
    pinned_cells: HashSet<usize>,
}

thread_local! {
    static INSPECTION_CONTEXT: RefCell<InspectionContext> = RefCell::new(InspectionContext::default());
}

/// 【终端】【查看保护】在作用域内启用折叠冻结和步骤固定，防止新内容收起当前查看的工具。
///
/// 参数:
/// - `freeze_fold`: 是否冻结自动折起已有未折叠详情
/// - `pinned_cells`: 用户展开或悬停中的步骤单元下标集合
/// - `render`: 作用域内执行的渲染闭包
///
/// 返回:
/// - 渲染闭包返回值
pub(crate) fn with_inspection_protection<R>(
    freeze_fold: bool,
    pinned_cells: &HashSet<usize>,
    render: impl FnOnce() -> R,
) -> R {
    let previous = INSPECTION_CONTEXT.with(|slot| {
        std::mem::replace(
            &mut *slot.borrow_mut(),
            InspectionContext {
                freeze_fold,
                pinned_cells: pinned_cells.clone(),
            },
        )
    });
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(render));
    INSPECTION_CONTEXT.with(|slot| {
        *slot.borrow_mut() = previous;
    });
    match outcome {
        Ok(value) => value,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

/// 【终端】【折叠冻结判断】检查当前是否处于暂停跟随或工具查看保护状态。
///
/// 返回:
/// - 处于查看保护时返回 true
fn folding_frozen() -> bool {
    INSPECTION_CONTEXT.with(|slot| slot.borrow().freeze_fold)
}

/// 【终端】【步骤固定判断】判断指定单元是否正在被用户查看或手动展开固定。
///
/// 参数:
/// - `cell`: 历史单元下标
///
/// 返回:
/// - 受保护返回 true
pub(super) fn is_cell_pinned(cell: usize) -> bool {
    INSPECTION_CONTEXT.with(|slot| slot.borrow().pinned_cells.contains(&cell))
}

/// 【终端】【详情窗口】记住已离开窗口的节点，避免定稿或收起后重新展开旧详情。
#[derive(Default)]
pub(in crate::render::transcript) struct RecentDetails {
    folded: HashSet<usize>,
}

impl RecentDetails {
    /// 【终端】【离屏收起】将已滚出底屏可视区的步骤登记为已折叠，使其回归默认折叠生命周期。
    ///
    /// 参数:
    /// - `cells`: 需收起的步骤单元下标集合
    ///
    /// 返回:
    /// - 是否有新增收起项
    pub(in crate::render::transcript) fn fold_cells(
        &mut self,
        cells: impl IntoIterator<Item = usize>,
    ) -> bool {
        let mut changed = false;
        for cell in cells {
            changed |= self.folded.insert(cell);
        }
        changed
    }

    /// 【终端】【详情窗口】从末尾向前渲染节点，只在较新内容占满十行且未受查看保护时自动收起旧详情。
    ///
    /// 参数:
    /// - `index`: 节点稳定下标
    /// - `newer_rows`: 累积的较新节点物理行数
    /// - `required`: 是否必须强制展开
    /// - `preview`: 是否默认保留预览，并用整块键控制全文展开
    /// - `expanded`: 手动展开/收起集合
    /// - `render`: 节点渲染闭包
    ///
    /// 返回:
    /// - 对应的点击段落键和已渲染节点
    pub(super) fn render(
        &mut self,
        index: usize,
        newer_rows: &mut usize,
        required: bool,
        preview: bool,
        expanded: &HashSet<ParagraphKey>,
        render: impl FnOnce(bool) -> RenderedNode,
    ) -> (ParagraphKey, RenderedNode) {
        let frozen = folding_frozen();
        let pinned = is_cell_pinned(index);
        // 1. 【终端】【详情窗口】用户查看或冻结折叠期间不自动收起尚未折叠的详情
        if *newer_rows >= DETAIL_WINDOW_ROWS && !required && !frozen && !pinned {
            self.folded.insert(index);
        }
        let recent = !self.folded.contains(&index);
        let active_key = ParagraphKey::activity(index, false);
        let history_key = ParagraphKey::whole(index);
        let key = if recent && !preview {
            active_key
        } else {
            history_key
        };

        // 2. 【终端】【状态互通】显式手动展开与手动收起优先级高于生命周期默认值
        let open = if required {
            true
        } else if expanded.contains(&history_key) {
            true
        } else if expanded.contains(&active_key) {
            false
        } else {
            recent
        };

        // 3. 【终端】【正文生成】执行渲染并累加较新行计数
        let node = render(open);
        *newer_rows = newer_rows.saturating_add(node.lines.len().max(1));
        (key, node)
    }
}
