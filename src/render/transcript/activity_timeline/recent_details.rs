use super::super::fullscreen_parts::ParagraphKey;
use super::node::RenderedNode;
use std::collections::HashSet;

pub(in crate::render::transcript) const DETAIL_WINDOW_ROWS: usize = 10;

/// 【终端】【详情窗口】记住已离开窗口的节点，避免定稿或收起后重新展开旧详情。
#[derive(Default)]
pub(in crate::render::transcript) struct RecentDetails {
    folded: HashSet<usize>,
}

impl RecentDetails {
    /// 【终端】【详情窗口】从末尾向前渲染节点，只在较新内容占满十行后自动收起旧详情。
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
        // 1. 【终端】【详情窗口】完整退出窗口才折叠，仍有一行可见的旧节点保留详情
        if *newer_rows >= DETAIL_WINDOW_ROWS && !required {
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
