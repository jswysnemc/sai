use std::collections::HashMap;

/// 同一调用重复多少次后开始提醒。
const WARN_THRESHOLD: usize = 2;

/// 同一个必然失败的调用重复多少次后停止执行。
///
/// 只对拒绝类结果生效：工具名不存在、参数畸形、被门禁挡下的调用重发多少次
/// 结果都一样，继续放行就是纯粹的空转。成功返回过的调用不适用此阈值。
const REJECTED_STOP_THRESHOLD: usize = 3;

/// 【Agent】【循环防护】识别模型反复发起同一工具调用的状态。
///
/// 长程任务里合法的重复很常见：轮询后台任务、等待文件生成、重读被外部改动的
/// 文件。硬性拒绝会误伤这类调用，因此成功过的重复只提醒不拦截，把判断权交回
/// 模型。真正需要拦截的是必然失败的重复——幻觉的工具名、畸形参数、被门禁挡
/// 下的调用，它们重发多少次结果都一样。
#[derive(Default)]
pub(super) struct RepeatGuard {
    counts: HashMap<(String, String), Observation>,
}

/// 单个 (工具名, 参数) 组合的观测状态。
#[derive(Default)]
struct Observation {
    /// 累计发起次数
    seen: usize,
    /// 累计被拒次数；与 `seen` 相等说明这个调用从未成功过
    rejected: usize,
}

/// 重复检测结论。
#[derive(Debug, Eq, PartialEq)]
pub(super) enum RepeatVerdict {
    /// 正常执行
    Allow,
    /// 执行，但在结果后追加提醒
    Warn { seen: usize },
    /// 该调用只会再次失败，不执行并要求换路径
    Stop { seen: usize },
}

impl RepeatGuard {
    /// 记录一次工具调用并给出处理结论。
    ///
    /// 参数:
    /// - `name`: 工具名称
    /// - `arguments`: 原始参数文本
    ///
    /// 返回:
    /// - 本次调用应当放行、提醒还是停止
    pub(super) fn observe(&mut self, name: &str, arguments: &str) -> RepeatVerdict {
        // 1. 参数按去空白后的文本比对，避免格式抖动导致漏判
        let entry = self.counts.entry(key_of(name, arguments)).or_default();
        entry.seen += 1;
        let seen = entry.seen;
        let never_succeeded = entry.rejected + 1 >= seen;
        // 2. 反复被拒的调用换多少次都是同样结果，停止执行并要求换路径
        if never_succeeded && entry.rejected >= REJECTED_STOP_THRESHOLD {
            return RepeatVerdict::Stop { seen };
        }
        // 3. 其余重复照常执行，只提示模型结果可能没有变化
        if seen > WARN_THRESHOLD {
            return RepeatVerdict::Warn { seen };
        }
        RepeatVerdict::Allow
    }

    /// 记录一次未进入执行的拒绝，用于识别必然失败的重复。
    ///
    /// 未知工具名、畸形参数、门禁拦截、权限拒绝都走这里：它们不产生工具结果，
    /// 但同样占用轮次，必须计入重复统计。
    ///
    /// 参数:
    /// - `name`: 工具名称
    /// - `arguments`: 原始参数文本
    ///
    /// 返回:
    /// - 无；仅累加拒绝计数
    pub(super) fn observe_rejected(&mut self, name: &str, arguments: &str) {
        let entry = self.counts.entry(key_of(name, arguments)).or_default();
        entry.rejected += 1;
    }
}

/// 构造重复比对使用的键。
///
/// 参数:
/// - `name`: 工具名称
/// - `arguments`: 原始参数文本
///
/// 返回:
/// - 工具名与归一化参数组成的键
fn key_of(name: &str, arguments: &str) -> (String, String) {
    (name.to_string(), normalize_arguments(arguments))
}

/// 归一化工具参数，用于重复比对。
///
/// 参数:
/// - `arguments`: 原始参数文本
///
/// 返回:
/// - 去除空白差异后的文本
fn normalize_arguments(arguments: &str) -> String {
    arguments.split_whitespace().collect::<String>()
}

/// 生成重复调用的提醒文本。
///
/// 措辞只陈述事实、不下达禁令：合法的轮询与重试同样会触发这条提醒，
/// 由模型自行判断这次重复是否必要。
///
/// 参数:
/// - `name`: 工具名称
/// - `seen`: 该调用累计次数
///
/// 返回:
/// - 追加在工具结果后的提醒
pub(super) fn warn_notice(name: &str, seen: usize) -> String {
    format!(
        "\n\n[repeat guard] 本轮已用相同参数调用 {name} {seen} 次。若这次是在等待状态变化，继续即可；否则请基于已有结果推进任务。"
    )
}

/// 生成停止执行的提示文本。
///
/// 参数:
/// - `name`: 工具名称
/// - `seen`: 该调用累计次数
///
/// 返回:
/// - 代替工具结果返回给模型的提示
pub(super) fn stop_notice(name: &str, seen: usize) -> String {
    format!(
        "[repeat guard] 已停止执行：本轮用相同参数调用 {name} {seen} 次，每次都被拒绝，从未成功返回过结果。同样的调用不会有不同结果，请改用其它工具或其它参数完成任务。"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 阈值以内的重复调用应当照常放行。
    #[test]
    fn allows_calls_below_the_warn_threshold() {
        let mut guard = RepeatGuard::default();
        for _ in 0..WARN_THRESHOLD {
            assert_eq!(
                guard.observe("read_file", r#"{"path":"a"}"#),
                RepeatVerdict::Allow
            );
        }
    }

    /// 超过提醒阈值后继续执行，但给出提醒。
    #[test]
    fn warns_after_the_threshold_is_exceeded() {
        let mut guard = RepeatGuard::default();
        for _ in 0..WARN_THRESHOLD {
            guard.observe("read_file", r#"{"path":"a"}"#);
        }
        assert_eq!(
            guard.observe("read_file", r#"{"path":"a"}"#),
            RepeatVerdict::Warn {
                seen: WARN_THRESHOLD + 1
            }
        );
    }

    /// 成功执行过的调用无论重复多少次都不会被停止。
    ///
    /// 轮询后台任务、等待文件生成都属于这一类：结果确实会变化。
    #[test]
    fn never_stops_calls_that_have_succeeded() {
        let mut guard = RepeatGuard::default();
        for _ in 0..20 {
            let verdict = guard.observe("run_command", r#"{"command":"tail log"}"#);
            assert_ne!(
                verdict,
                RepeatVerdict::Stop { seen: 0 },
                "成功过的调用不应被停止"
            );
            assert!(matches!(
                verdict,
                RepeatVerdict::Allow | RepeatVerdict::Warn { .. }
            ));
        }
    }

    /// 始终被拒绝的调用达到阈值后停止执行。
    #[test]
    fn stops_calls_that_only_ever_get_rejected() {
        let mut guard = RepeatGuard::default();
        // 每次发起后都被门禁拒绝，从未产生工具结果
        for _ in 0..REJECTED_STOP_THRESHOLD {
            assert!(matches!(
                guard.observe("hallucinated", "{}"),
                RepeatVerdict::Allow | RepeatVerdict::Warn { .. }
            ));
            guard.observe_rejected("hallucinated", "{}");
        }
        assert_eq!(
            guard.observe("hallucinated", "{}"),
            RepeatVerdict::Stop {
                seen: REJECTED_STOP_THRESHOLD + 1
            }
        );
    }

    /// 曾经成功过的调用即使后续偶发失败也不会被停止。
    #[test]
    fn a_single_success_disarms_the_stop_verdict() {
        let mut guard = RepeatGuard::default();
        // 第一次成功执行，不记录拒绝
        guard.observe("write_file", r#"{"path":"a"}"#);
        // 之后连续被拒
        for _ in 0..REJECTED_STOP_THRESHOLD + 2 {
            let verdict = guard.observe("write_file", r#"{"path":"a"}"#);
            assert!(matches!(
                verdict,
                RepeatVerdict::Allow | RepeatVerdict::Warn { .. }
            ));
            guard.observe_rejected("write_file", r#"{"path":"a"}"#);
        }
    }

    /// 参数不同的调用互不影响。
    #[test]
    fn tracks_each_argument_set_separately() {
        let mut guard = RepeatGuard::default();
        for _ in 0..=REJECTED_STOP_THRESHOLD {
            guard.observe("read_file", r#"{"path":"a"}"#);
            guard.observe_rejected("read_file", r#"{"path":"a"}"#);
        }
        assert_eq!(
            guard.observe("read_file", r#"{"path":"b"}"#),
            RepeatVerdict::Allow
        );
    }

    /// 仅有空白差异的参数视为同一调用。
    #[test]
    fn ignores_whitespace_differences_in_arguments() {
        // 三种写法只有空白差异，应当累计为同一个调用
        let mut guard = RepeatGuard::default();
        for _ in 0..WARN_THRESHOLD {
            guard.observe("read_file", r#"{"path":"a"}"#);
        }

        assert_eq!(
            guard.observe("read_file", "{\n  \"path\": \"a\"\n}"),
            RepeatVerdict::Warn {
                seen: WARN_THRESHOLD + 1
            }
        );
    }
}
