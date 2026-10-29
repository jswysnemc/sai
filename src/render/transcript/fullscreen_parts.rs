//! 执行链中的稳定段落键，区分分组、历史步骤与当前步骤。

/// 段落在 cell 中的位置：整块，或执行链的分组、较早步骤与当前步骤。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum ParagraphPart {
    /// 整个 cell 作为一段（思考、diff、普通工具等）
    Whole,
    /// 线性执行链的分组标题，默认展开，集合中存在此键时收起
    ActivityGroup,
    /// 执行链中的较早步骤，默认收起，集合中存在此键时展开
    OlderActivity,
    /// 当前步骤默认展开，集合中存在此键时暂时收起详情
    Active,
}

/// 段落键：cell 下标与段落位置。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct ParagraphKey {
    /// cell 下标，流式尾部使用下一条记录的下标
    pub(crate) cell: usize,
    pub(crate) part: ParagraphPart,
}

impl ParagraphKey {
    /// 【终端】【旧步骤键】构造独立于整组和子节点的历史折叠键。
    /// 参数: `cell` 为分组起始下标；返回: 默认收起的较早步骤键。
    pub(super) fn older_activity(cell: usize) -> Self {
        Self {
            cell,
            part: ParagraphPart::OlderActivity,
        }
    }

    /// 【终端】【当前步骤】区分默认展开的实时步骤与手动展开的历史节点。
    /// 参数: 无；返回: 是否为当前活动步骤键。
    pub(crate) fn is_active(self) -> bool {
        matches!(self.part, ParagraphPart::Active)
    }

    /// 【终端】【执行链键】构造默认展开的分组或当前步骤键。
    /// 参数: `cell` 为步骤下标，`group` 表示是否为分组；返回: 独立折叠键。
    pub(super) fn activity(cell: usize, group: bool) -> Self {
        Self {
            cell,
            part: if group {
                ParagraphPart::ActivityGroup
            } else {
                ParagraphPart::Active
            },
        }
    }

    /// 【全屏视图】【段落键】构造整块段落键。
    /// @param cell 为 cell 下标
    /// @returns 段落键
    pub(crate) fn whole(cell: usize) -> Self {
        Self {
            cell,
            part: ParagraphPart::Whole,
        }
    }
}
