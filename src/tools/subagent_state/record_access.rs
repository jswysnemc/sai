use super::*;

/// 取出仍在运行中的记录,终态记录返回空。
pub(super) fn running_record<'map>(
    subagents: &'map mut HashMap<String, SubagentRecord>,
    id: &str,
) -> Option<&'map mut SubagentRecord> {
    subagents
        .get_mut(id)
        .filter(|record| record.snapshot.status == "running")
}
