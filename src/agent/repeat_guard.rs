use std::collections::HashMap;

/// 同一调用重复多少次后开始提醒。
const WARN_THRESHOLD: usize = 2;

/// 同一个必然失败的调用重复多少次后停止执行。
///
/// 只对拒绝类结果生效：工具名不存在、参数畸形、被门禁挡下的调用重发多少次
/// 结果都一样，继续放行就是纯粹的空转。
const REJECTED_STOP_THRESHOLD: usize = 3;

/// 同一参数组合无论成败累计多少次后强制停止。
///
/// 弱模型会无视提醒，把已成功的安装、查询再跑几十遍。轮询后台任务通常
/// 三两次就能换超时或改参数；超过这个硬上限后继续执行只会空转。
const IDENTICAL_STOP_THRESHOLD: usize = 4;

/// 【Agent】【循环防护】识别模型反复发起同一工具调用的状态。
///
/// 长程任务里合法的重复很常见：轮询后台任务、等待文件生成、重读被外部改动的
/// 文件。前几次成功重复只提醒不拦截。真正需要立刻拦截的是必然失败的重复；
/// 成功过的相同调用在硬上限之后同样停止，避免模型看不见结果或无视提醒时
/// 无限重装、重查。
#[derive(Default)]
pub(crate) struct RepeatGuard {
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RepeatVerdict {
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
    pub(crate) fn observe(&mut self, name: &str, arguments: &str) -> RepeatVerdict {
        // 1. JSON 按字段排序比对，其余参数去空白，避免格式抖动漏判
        let entry = self.counts.entry(key_of(name, arguments)).or_default();
        entry.seen += 1;
        let seen = entry.seen;
        let never_succeeded = entry.rejected + 1 >= seen;
        // 2. 反复被拒的调用换多少次都是同样结果，停止执行并要求换路径
        if never_succeeded && entry.rejected >= REJECTED_STOP_THRESHOLD {
            return RepeatVerdict::Stop { seen };
        }
        // 3. 相同参数累计超过硬上限后一律停止，挡住成功空转
        if seen > IDENTICAL_STOP_THRESHOLD {
            return RepeatVerdict::Stop { seen };
        }
        // 4. 其余重复照常执行，只提示模型结果可能没有变化
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
    pub(crate) fn observe_rejected(&mut self, name: &str, arguments: &str) {
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
/// - JSON 按字段名排序后的文本；非 JSON 则去掉空白
fn normalize_arguments(arguments: &str) -> String {
    // 与实际执行的入参保持同一解析规则，否则同一调用换个残片就绕过重复防护
    super::first_json_object(arguments)
        .map(|value| canonical_json(&value))
        .unwrap_or_else(|_| arguments.split_whitespace().collect::<String>())
}

/// 把 JSON 转成字段名排序后的稳定文本。
///
/// 参数:
/// - `value`: 已解析的 JSON
///
/// 返回:
/// - 字段排序后的 JSON 文本
fn canonical_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(map) => {
            let mut keys = map.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            let fields = keys
                .into_iter()
                .map(|key| {
                    let encoded_key =
                        serde_json::to_string(&key).unwrap_or_else(|_| format!("\"{key}\""));
                    format!("{encoded_key}:{}", canonical_json(&map[&key]))
                })
                .collect::<Vec<_>>()
                .join(",");
            format!("{{{fields}}}")
        }
        serde_json::Value::Array(items) => {
            let items = items
                .iter()
                .map(canonical_json)
                .collect::<Vec<_>>()
                .join(",");
            format!("[{items}]")
        }
        other => serde_json::to_string(other).unwrap_or_default(),
    }
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
pub(crate) fn warn_notice(name: &str, seen: usize) -> String {
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
pub(crate) fn stop_notice(name: &str, seen: usize) -> String {
    format!(
        "[repeat guard] 已停止执行：本轮用相同参数调用 {name} {seen} 次。继续重复不会推进任务，请基于已有结果换路径或向用户汇报。"
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

    /// 成功执行过的调用在硬上限以内只提醒，达到上限后停止。
    #[test]
    fn stops_identical_successful_calls_after_the_hard_cap() {
        let mut guard = RepeatGuard::default();
        for _ in 0..IDENTICAL_STOP_THRESHOLD {
            let verdict = guard.observe("run_command", r#"{"command":"pacman -U pkg"}"#);
            assert!(matches!(
                verdict,
                RepeatVerdict::Allow | RepeatVerdict::Warn { .. }
            ));
        }
        assert_eq!(
            guard.observe("run_command", r#"{"command":"pacman -U pkg"}"#),
            RepeatVerdict::Stop {
                seen: IDENTICAL_STOP_THRESHOLD + 1
            }
        );
    }

    /// 尾随残片不能绕过重复防护：归一化要与实际执行的入参一致。
    #[test]
    fn trailing_content_normalizes_to_the_same_key() {
        assert_eq!(
            normalize_arguments(r#"{"path":"a"} 残余片段"#),
            normalize_arguments(r#"{"path":"a"}"#)
        );
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

    /// 曾经成功过的调用不会被拒绝阈值提前拦住，但仍受相同参数硬上限约束。
    #[test]
    fn a_single_success_disarms_rejected_stop_but_not_the_hard_cap() {
        let mut guard = RepeatGuard::default();
        // 第一次成功执行，不记录拒绝
        guard.observe("write_file", r#"{"path":"a"}"#);
        // 之后连续被拒，在硬上限以内仍放行
        for _ in 1..IDENTICAL_STOP_THRESHOLD {
            let verdict = guard.observe("write_file", r#"{"path":"a"}"#);
            assert!(matches!(
                verdict,
                RepeatVerdict::Allow | RepeatVerdict::Warn { .. }
            ));
            guard.observe_rejected("write_file", r#"{"path":"a"}"#);
        }
        assert_eq!(
            guard.observe("write_file", r#"{"path":"a"}"#),
            RepeatVerdict::Stop {
                seen: IDENTICAL_STOP_THRESHOLD + 1
            }
        );
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

    /// JSON 字段顺序不同的参数视为同一调用。
    #[test]
    fn ignores_json_key_order_in_arguments() {
        let mut guard = RepeatGuard::default();
        for _ in 0..WARN_THRESHOLD {
            guard.observe(
                "background_command",
                r#"{"action":"output","task_id":"t1","tail_lines":40}"#,
            );
        }

        assert_eq!(
            guard.observe(
                "background_command",
                r#"{"tail_lines":40,"task_id":"t1","action":"output"}"#,
            ),
            RepeatVerdict::Warn {
                seen: WARN_THRESHOLD + 1
            }
        );
    }
}
