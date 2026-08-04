use super::super::model::MemoryHit;

/// 允许注入的最低综合得分。
///
/// 旧实现把召回结果一股脑注入，再用「可能相关也可能不相关」免责。
/// 这句话教会模型忽略整块内容，准确召回的部分跟着一起废掉。
/// 这里改为宁缺毋滥：低于阈值直接不注入，注入的就断言它相关。
pub const INJECTION_SCORE_FLOOR: f64 = 0.35;

/// 单次注入的记忆条数上限。
pub const MAX_INJECTED_MEMORIES: usize = 6;

/// 筛选出值得注入的记忆。
///
/// 参数:
/// - `hits`: 召回命中，可未排序
///
/// 返回:
/// - 按得分降序、通过阈值且数量受限的命中
pub fn select_for_injection(mut hits: Vec<MemoryHit>) -> Vec<MemoryHit> {
    hits.retain(|hit| hit.score >= INJECTION_SCORE_FLOOR);
    hits.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    hits.truncate(MAX_INJECTED_MEMORIES);
    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::model::{MemoryItem, MemoryKind, MemoryScope};

    fn hit(id: i64, score: f64) -> MemoryHit {
        MemoryHit {
            item: MemoryItem {
                id,
                kind: MemoryKind::Preference,
                scope: MemoryScope::Global,
                content: format!("记忆 {id}"),
                salience: 1.0,
                tags: Vec::new(),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:00:00Z".to_string(),
            },
            relevance: score,
            strength: 1.0,
            score,
        }
    }

    /// 验证低于阈值的命中不被注入。
    #[test]
    fn drops_hits_below_the_floor() {
        let selected = select_for_injection(vec![hit(1, 0.9), hit(2, 0.1)]);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].item.id, 1);
    }

    /// 验证结果按得分降序排列。
    #[test]
    fn orders_hits_by_score() {
        let selected = select_for_injection(vec![hit(1, 0.5), hit(2, 0.9), hit(3, 0.7)]);
        assert_eq!(
            selected.iter().map(|hit| hit.item.id).collect::<Vec<_>>(),
            vec![2, 3, 1]
        );
    }

    /// 验证注入条数受上限约束。
    #[test]
    fn caps_the_number_of_injected_memories() {
        let hits = (1..=20).map(|id| hit(id, 0.9)).collect();
        assert_eq!(select_for_injection(hits).len(), MAX_INJECTED_MEMORIES);
    }

    /// 验证全部低于阈值时不注入任何内容。
    #[test]
    fn yields_nothing_when_every_hit_is_weak() {
        assert!(select_for_injection(vec![hit(1, 0.2), hit(2, 0.1)]).is_empty());
    }
}
