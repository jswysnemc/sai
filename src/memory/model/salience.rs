/// 显著性与时间衰减的计算。
///
/// 旧实现在每次召回命中时调用 `reinforce` 提升强度，形成正反馈：
/// 高频关键词的记忆越滚越强，长尾记忆持续衰减直到消失。这里改为纯时间衰减，
/// 召回不改变强度——一条记忆的价值由它被写入时的显著性决定，不由它被搜到的
/// 次数决定。

/// 记忆强度的下限，低于该值视为已遗忘。
pub const FORGOTTEN_THRESHOLD: f64 = 0.05;

/// 计算按时间衰减后的记忆强度。
///
/// 采用半衰期模型：经过一个半衰期，强度减半。
///
/// 参数:
/// - `initial_strength`: 写入时的显著性，取值 0.0 到 1.0
/// - `age_days`: 距写入的天数
/// - `half_life_days`: 半衰期天数
///
/// 返回:
/// - 当前强度，取值 0.0 到 1.0
pub fn decayed_strength(initial_strength: f64, age_days: f64, half_life_days: f64) -> f64 {
    if age_days <= 0.0 {
        return initial_strength.clamp(0.0, 1.0);
    }
    // 半衰期非正时不衰减，避免除零与指数爆炸
    let half_life = half_life_days.max(0.1);
    let decayed = initial_strength * 2f64.powf(-age_days / half_life);
    decayed.clamp(0.0, 1.0)
}

/// 判断记忆是否已衰减到可以清理的程度。
///
/// 参数:
/// - `strength`: 当前强度
///
/// 返回:
/// - 可清理时为 true
pub fn is_forgotten(strength: f64) -> bool {
    strength < FORGOTTEN_THRESHOLD
}

/// 计算召回排序使用的综合得分。
///
/// 检索相关度决定「是不是这条」，记忆强度决定「这条还重不重要」，
/// 两者相乘避免高相关但已过期的记忆挤掉低相关但仍有效的偏好。
///
/// 参数:
/// - `relevance`: 检索相关度，取值 0.0 到 1.0
/// - `strength`: 当前记忆强度，取值 0.0 到 1.0
///
/// 返回:
/// - 综合得分，取值 0.0 到 1.0
pub fn ranking_score(relevance: f64, strength: f64) -> f64 {
    (relevance.clamp(0.0, 1.0) * strength.clamp(0.0, 1.0)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证一个半衰期后强度减半。
    #[test]
    fn one_half_life_halves_the_strength() {
        let strength = decayed_strength(1.0, 30.0, 30.0);
        assert!((strength - 0.5).abs() < 1e-9, "实际强度 {strength}");
    }

    /// 验证刚写入的记忆不衰减。
    #[test]
    fn fresh_memories_keep_their_initial_strength() {
        assert_eq!(decayed_strength(0.8, 0.0, 30.0), 0.8);
        assert_eq!(decayed_strength(0.8, -1.0, 30.0), 0.8);
    }

    /// 验证衰减单调递减且不会为负。
    #[test]
    fn decay_is_monotonic_and_bounded() {
        let mut previous = 1.0;
        for days in [1.0, 10.0, 100.0, 1_000.0] {
            let current = decayed_strength(1.0, days, 30.0);
            assert!(current < previous, "{days} 天时强度未下降");
            assert!(current >= 0.0);
            previous = current;
        }
    }

    /// 验证长期未召回的记忆最终被判定为遗忘。
    #[test]
    fn long_unused_memories_are_eventually_forgotten() {
        assert!(is_forgotten(decayed_strength(1.0, 365.0, 30.0)));
        assert!(!is_forgotten(decayed_strength(1.0, 1.0, 30.0)));
    }

    /// 验证排序得分同时受相关度与强度约束。
    ///
    /// 高相关但已过期的记忆不应排在低相关但仍强的记忆之前。
    #[test]
    fn ranking_balances_relevance_against_strength() {
        let stale_but_relevant = ranking_score(0.9, 0.1);
        let weaker_but_fresh = ranking_score(0.5, 0.9);
        assert!(weaker_but_fresh > stale_but_relevant);
    }

    /// 验证得分被钳制在有效区间。
    #[test]
    fn ranking_score_stays_within_bounds() {
        assert_eq!(ranking_score(2.0, 2.0), 1.0);
        assert_eq!(ranking_score(-1.0, 0.5), 0.0);
    }
}
