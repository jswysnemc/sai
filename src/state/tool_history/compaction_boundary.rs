use super::model::ToolExchangeRecord;

/// 【上下文】【工具子轮边界】把覆盖前缀向前对齐，保证未覆盖结果所在子轮完整保留
/// 参数: exchanges 为顺序工具记录，requested 为期望覆盖条数；返回不超过 requested 的完整子轮边界
pub(in crate::state) fn aligned_compaction_prefix(
    exchanges: &[ToolExchangeRecord],
    requested: usize,
) -> usize {
    let mut boundary = requested.min(exchanges.len());
    if boundary == exchanges.len() {
        return boundary;
    }
    let retained_round = exchanges[boundary].call.assistant_round;
    while boundary > 0 && exchanges[boundary - 1].call.assistant_round == retained_round {
        boundary -= 1;
    }
    boundary
}

/// 【上下文】【工具子轮保留】读取边界后的完整工具子轮，兼容历史未对齐 checkpoint
/// 参数: exchanges 为顺序记录，requested 为已记录覆盖条数；返回保留记录切片
pub(super) fn retained_exchanges(
    exchanges: &[ToolExchangeRecord],
    requested: usize,
) -> &[ToolExchangeRecord] {
    &exchanges[aligned_compaction_prefix(exchanges, requested)..]
}
