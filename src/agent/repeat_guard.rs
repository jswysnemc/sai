use std::collections::HashMap;

/// 同一调用重复多少次后开始警告。
const WARN_THRESHOLD: usize = 2;

/// 同一调用重复多少次后拒绝执行。
///
/// 同一份参数读第三次时结果已经确定不会变化，再放行只是浪费轮次。
const BLOCK_THRESHOLD: usize = 3;

/// 【Agent】【循环防护】识别模型反复发起同一工具调用的状态。
///
/// 模型偶尔会对同一个参数反复调用同一工具：每次都成功、每次都拿到结果，
/// 却仍然重复请求。`tools.max_rounds` 为 0 时没有任何上限，这种空转会
/// 一直持续到用户手动中断。这里按 (工具名, 参数) 计数，重复到阈值后先
/// 警告、再拒绝执行，把控制权交回模型。
#[derive(Default)]
pub(super) struct RepeatGuard {
    counts: HashMap<(String, String), usize>,
}

/// 重复检测结论。
#[derive(Debug, Eq, PartialEq)]
pub(super) enum RepeatVerdict {
    /// 正常执行
    Allow,
    /// 执行，但在结果后追加提醒
    Warn { seen: usize },
    /// 不执行，直接返回提示
    Block { seen: usize },
}

impl RepeatGuard {
    /// 记录一次工具调用并给出处理结论。
    ///
    /// 参数:
    /// - `name`: 工具名称
    /// - `arguments`: 原始参数文本
    ///
    /// 返回:
    /// - 本次调用应当放行、警告还是拒绝
    pub(super) fn observe(&mut self, name: &str, arguments: &str) -> RepeatVerdict {
        // 1. 参数按去空白后的文本比对，避免格式抖动导致漏判
        let key = (name.to_string(), normalize_arguments(arguments));
        let seen = self.counts.entry(key).or_insert(0);
        *seen += 1;
        let seen = *seen;
        // 2. 达到拒绝阈值后不再执行，只把情况告诉模型
        if seen > BLOCK_THRESHOLD {
            return RepeatVerdict::Block { seen };
        }
        // 3. 达到警告阈值后照常执行，但提示模型已经拿过同样的结果
        if seen > WARN_THRESHOLD {
            return RepeatVerdict::Warn { seen };
        }
        RepeatVerdict::Allow
    }
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

/// 生成重复调用的警告文本。
///
/// 参数:
/// - `name`: 工具名称
/// - `seen`: 该调用累计次数
///
/// 返回:
/// - 追加在工具结果后的提醒
pub(super) fn warn_notice(name: &str, seen: usize) -> String {
    format!(
        "\n\n[repeat guard] 本轮已用相同参数调用 {name} {seen} 次，结果没有变化。请基于已有结果继续，不要再次发起同样的调用。"
    )
}

/// 生成拒绝执行的提示文本。
///
/// 参数:
/// - `name`: 工具名称
/// - `seen`: 该调用累计次数
///
/// 返回:
/// - 代替工具结果返回给模型的提示
pub(super) fn block_notice(name: &str, seen: usize) -> String {
    format!(
        "[repeat guard] 已拒绝执行：本轮用相同参数调用 {name} 达到 {seen} 次，此前每次都已成功返回结果。请改用其它参数，或直接根据已获得的信息回答用户。"
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
            assert_eq!(guard.observe("read_file", r#"{"path":"a"}"#), RepeatVerdict::Allow);
        }
    }

    /// 超过警告阈值后继续执行，但给出提醒。
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

    /// 超过拒绝阈值后不再执行。
    #[test]
    fn blocks_after_the_block_threshold() {
        let mut guard = RepeatGuard::default();
        for _ in 0..BLOCK_THRESHOLD {
            guard.observe("read_file", r#"{"path":"a"}"#);
        }
        assert_eq!(
            guard.observe("read_file", r#"{"path":"a"}"#),
            RepeatVerdict::Block {
                seen: BLOCK_THRESHOLD + 1
            }
        );
    }

    /// 参数不同的调用互不影响。
    #[test]
    fn tracks_each_argument_set_separately() {
        let mut guard = RepeatGuard::default();
        for _ in 0..=BLOCK_THRESHOLD {
            guard.observe("read_file", r#"{"path":"a"}"#);
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
