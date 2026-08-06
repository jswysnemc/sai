use super::super::model::MemoryCandidate;
#[cfg(test)]
use super::super::model::MemoryKind;

/// 记忆正文的最大字符数，超出说明抽取没有真正提炼。
const MAX_CONTENT_CHARS: usize = 280;

/// 记忆正文的最小字符数，过短的陈述没有检索价值。
const MIN_CONTENT_CHARS: usize = 6;

/// 准入判定结论。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapturePolicyVerdict {
    /// 允许写入
    Accept,
    /// 拒绝写入及其原因
    Reject(RejectReason),
}

/// 拒绝写入的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    /// 正文过短或过长
    ContentLength,
    /// 显著性低于该类型的门槛
    BelowSalienceFloor,
    /// 正文只是复述对话动作，不含可复用信息
    ConversationalNoise,
}

/// 判断一条候选记忆是否值得写入。
///
/// 旧实现每轮无条件写入，只用寒暄词表和长度做过滤，库里堆的是对话流水账。
/// 这里按类型设置显著性门槛，并挡掉复述对话动作的伪记忆。
///
/// 参数:
/// - `candidate`: 已归一化的候选记忆
///
/// 返回:
/// - 准入结论
pub fn evaluate(candidate: &MemoryCandidate) -> CapturePolicyVerdict {
    let chars = candidate.content.chars().count();
    // 1. 长度不在合理区间说明抽取失败：要么没提炼，要么是碎片
    if !(MIN_CONTENT_CHARS..=MAX_CONTENT_CHARS).contains(&chars) {
        return CapturePolicyVerdict::Reject(RejectReason::ContentLength);
    }
    // 2. 复述对话动作的内容没有长期价值，先于显著性判定挡掉
    if is_conversational_noise(&candidate.content) {
        return CapturePolicyVerdict::Reject(RejectReason::ConversationalNoise);
    }
    // 3. 按类型门槛判定显著性，往事的门槛最严
    if candidate.salience < candidate.kind.capture_floor() {
        return CapturePolicyVerdict::Reject(RejectReason::BelowSalienceFloor);
    }
    CapturePolicyVerdict::Accept
}

/// 判断正文是否只是在复述这轮对话发生了什么。
///
/// 这类内容形如「用户询问了 X」「助手回答了 Y」，读回来对后续任务毫无帮助，
/// 却正是旧实现产出的主要形态。
///
/// 参数:
/// - `content`: 记忆正文
///
/// 返回:
/// - 属于对话噪声时为 true
fn is_conversational_noise(content: &str) -> bool {
    const NOISE_PREFIXES: &[&str] = &[
        "用户询问",
        "用户问",
        "用户提问",
        "用户要求",
        "用户说",
        "助手回答",
        "助手回复",
        "助手解释",
        "助手说明",
        "本轮对话",
        "这次对话",
        "user asked",
        "user requested",
        "the assistant replied",
        "the assistant explained",
    ];
    let normalized = content.trim().to_lowercase();
    NOISE_PREFIXES
        .iter()
        .any(|prefix| normalized.starts_with(&prefix.to_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::model::MemoryScope;

    fn candidate(kind: MemoryKind, content: &str, salience: f64) -> MemoryCandidate {
        MemoryCandidate {
            kind,
            scope: MemoryScope::Global,
            content: content.to_string(),
            salience,
            tags: Vec::new(),
        }
    }

    /// 验证显著的偏好可以写入。
    #[test]
    fn accepts_a_salient_preference() {
        let verdict = evaluate(&candidate(
            MemoryKind::Preference,
            "用户在 Node 项目中一律使用 pnpm，不使用 npm",
            0.8,
        ));
        assert_eq!(verdict, CapturePolicyVerdict::Accept);
    }

    /// 验证低显著性的往事被拒。
    ///
    /// 这正是旧实现堆成流水账的那一类内容。
    #[test]
    fn rejects_a_low_salience_episode() {
        let verdict = evaluate(&candidate(
            MemoryKind::Episode,
            "帮用户看了一下配置文件的内容",
            0.4,
        ));
        assert_eq!(
            verdict,
            CapturePolicyVerdict::Reject(RejectReason::BelowSalienceFloor)
        );
    }

    /// 验证同样显著性下偏好通过而往事被拒。
    #[test]
    fn the_floor_differs_by_kind() {
        assert_eq!(
            evaluate(&candidate(
                MemoryKind::Preference,
                "用户偏好紧凑的界面布局",
                0.5
            )),
            CapturePolicyVerdict::Accept
        );
        assert_eq!(
            evaluate(&candidate(MemoryKind::Episode, "调整了界面布局的间距", 0.5)),
            CapturePolicyVerdict::Reject(RejectReason::BelowSalienceFloor)
        );
    }

    /// 验证复述对话动作的内容被挡掉。
    #[test]
    fn rejects_content_that_merely_narrates_the_exchange() {
        for content in [
            "用户询问了如何配置代理，助手给出了步骤",
            "助手回答了关于编译错误的问题",
            "User asked about the build failure",
        ] {
            assert_eq!(
                evaluate(&candidate(MemoryKind::Fact, content, 0.95)),
                CapturePolicyVerdict::Reject(RejectReason::ConversationalNoise),
                "未拦下对话噪声: {content}"
            );
        }
    }

    /// 验证过短与过长的正文都被拒。
    #[test]
    fn rejects_content_outside_the_length_window() {
        assert_eq!(
            evaluate(&candidate(MemoryKind::Fact, "短", 0.9)),
            CapturePolicyVerdict::Reject(RejectReason::ContentLength)
        );
        let long = "长".repeat(MAX_CONTENT_CHARS + 1);
        assert_eq!(
            evaluate(&candidate(MemoryKind::Fact, &long, 0.9)),
            CapturePolicyVerdict::Reject(RejectReason::ContentLength)
        );
    }
}
