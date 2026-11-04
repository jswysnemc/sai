use super::{control::PasteControl, WindowsPasteKey};
use std::time::Instant;

/// 【终端】【粘贴回放】仅保留批量匹配所需的原生事件语义
#[derive(Clone, Copy, Debug)]
pub(super) enum ReplayRecord {
    Release,
    Unit(u16),
    Enter,
    LineFeed,
    Boundary,
}

/// 【终端】【粘贴回放】可安全消费的事件前缀和对应 UTF-8 字节数
#[derive(Debug, Default, Eq, PartialEq)]
pub(super) struct MatchedPrefix {
    pub(super) records: usize,
    pub(super) bytes: usize,
}

/// 【终端】【粘贴回放】匹配完整 Unicode 字符，不越过后续按键或半个代理对
/// 参数: text 为待消费的粘贴正文，records 为原生事件摘要；返回匹配前缀
#[cfg(test)]
pub(super) fn matching_prefix(
    text: &str,
    records: impl IntoIterator<Item = ReplayRecord>,
) -> MatchedPrefix {
    matching_prefix_with_control(text, &PasteControl::default(), Instant::now(), records).0
}

/// 【终端】【粘贴回放】与逐事件输入共用原始换行配对，返回可提交的控制状态
/// 参数: text 为未消费原文，control 为当前配对，now 为读取时间，records 为原生事件
/// 返回: 可消费前缀及其末尾配对状态
pub(super) fn matching_prefix_with_control(
    text: &str,
    control: &PasteControl,
    now: Instant,
    records: impl IntoIterator<Item = ReplayRecord>,
) -> (MatchedPrefix, PasteControl) {
    let mut matched = MatchedPrefix::default();
    let mut control = control.clone();
    let mut low_surrogate = false;
    for (index, record) in records.into_iter().enumerate() {
        let remaining = &text[matched.bytes..];
        if remaining.is_empty() && !control.has_pending_lf(now) {
            break;
        }
        let unit = match record {
            ReplayRecord::Boundary => break,
            ReplayRecord::Release => {
                if !low_surrogate {
                    matched.records = index + 1;
                }
                continue;
            }
            ReplayRecord::Unit(unit) => unit,
            ReplayRecord::Enter => b'\r' as u16,
            ReplayRecord::LineFeed => b'\n' as u16,
        };
        // 1. 【终端】【原始换行】CRLF 可以由两个事件或一个规范化事件完整消费
        if !low_surrogate && matches!(unit, 9 | 10 | 13) {
            let key = match unit {
                9 => WindowsPasteKey::Tab,
                10 => WindowsPasteKey::LineFeed,
                _ => WindowsPasteKey::Enter,
            };
            let Some(bytes) = control.matching_bytes(remaining, key, now) else {
                break;
            };
            matched.bytes += bytes;
            matched.records = index + 1;
            continue;
        }
        let Some(expected) = remaining.chars().next() else {
            break;
        };
        let mut encoded = [0; 2];
        let expected_units = expected.encode_utf16(&mut encoded);
        if expected_units[usize::from(low_surrogate)] != unit {
            break;
        }
        // 1. 【终端】【粘贴回放】代理对完整到达后才提交，留给普通读取器的事件必须完整
        if expected_units.len() == 2 && !low_surrogate {
            low_surrogate = true;
            continue;
        }
        low_surrogate = false;
        let _ = control.matching_bytes(remaining, WindowsPasteKey::Char(expected), now);
        matched.bytes += expected.len_utf8();
        matched.records = index + 1;
    }
    (matched, control)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【终端】【ConPTY 回归】原始 CRLF 消费成一个正文换行，后续普通按键保持在队列
    /// 参数: 无；返回无，消费边界或换行配对错误时失败
    #[test]
    fn raw_crlf_prefix_preserves_followup_keys() {
        let text = "a\r\nb\r\n";
        let (matched, control) = matching_prefix_with_control(
            text,
            &PasteControl::default(),
            Instant::now(),
            [
                ReplayRecord::Unit(b'a' as u16),
                ReplayRecord::Enter,
                ReplayRecord::LineFeed,
                ReplayRecord::Unit(b'b' as u16),
                ReplayRecord::Enter,
                ReplayRecord::LineFeed,
                ReplayRecord::Unit(b'!' as u16),
            ],
        );
        assert_eq!(
            matched,
            MatchedPrefix {
                records: 6,
                bytes: text.len()
            }
        );
        assert!(!control.has_pending_lf(Instant::now()));
    }

    /// 【终端】【ConPTY 回归】终端把 CRLF 合并成普通回车时，独立空行仍完整消费
    /// 参数: 无；返回无，空行消失或回放中断时失败
    #[test]
    fn normalized_crlf_keeps_blank_lines() {
        let text = "a\r\n\r\nb";
        assert_eq!(
            matching_prefix(
                text,
                [
                    ReplayRecord::Unit(b'a' as u16),
                    ReplayRecord::Enter,
                    ReplayRecord::Enter,
                    ReplayRecord::Unit(b'b' as u16),
                ]
            ),
            MatchedPrefix {
                records: 4,
                bytes: text.len()
            }
        );
    }

    /// 【终端】【ConPTY 回归】CR 与 LF 分批到达时保留配对，超过窗口后保留用户控制键
    /// 参数: 无；返回无，跨批状态或后续按键边界错误时失败
    #[test]
    fn crlf_pair_survives_batches_and_expires_before_followup() {
        let now = Instant::now();
        let (first, control) = matching_prefix_with_control(
            "\r\n中",
            &PasteControl::default(),
            now,
            [ReplayRecord::Enter],
        );
        assert_eq!(
            first,
            MatchedPrefix {
                records: 1,
                bytes: 2
            }
        );
        let (second, _) = matching_prefix_with_control(
            "中",
            &control,
            now + std::time::Duration::from_millis(1),
            [ReplayRecord::LineFeed, ReplayRecord::Unit('中' as u16)],
        );
        assert_eq!(
            second,
            MatchedPrefix {
                records: 2,
                bytes: 3
            }
        );
        let (expired, _) = matching_prefix_with_control(
            "",
            &control,
            now + std::time::Duration::from_millis(121),
            [ReplayRecord::LineFeed],
        );
        assert_eq!(expired, MatchedPrefix::default());
    }

    /// 【终端】【粘贴回归】正文结束立即停止，后续相同字符也属于用户输入；无参数，返回无
    #[test]
    fn stops_at_paste_end_and_preserves_followup_text() {
        let matched = matching_prefix("中a", "中aa".encode_utf16().map(ReplayRecord::Unit));
        assert_eq!(
            matched,
            MatchedPrefix {
                records: 2,
                bytes: 4
            }
        );
    }

    /// 【终端】【粘贴回归】快捷键、窗口事件及不匹配字符保持原顺序；无参数，返回无
    #[test]
    fn stops_before_control_events_and_mismatches() {
        for boundary in [ReplayRecord::Boundary, ReplayRecord::Unit(b'!' as u16)] {
            let matched = matching_prefix("abc", [ReplayRecord::Unit(b'a' as u16), boundary]);
            assert_eq!(
                matched,
                MatchedPrefix {
                    records: 1,
                    bytes: 1
                }
            );
        }
    }

    /// 【终端】【粘贴回归】代理对可跨释放事件，半个代理对不得提前消费；无参数，返回无
    #[test]
    fn keeps_partial_surrogate_pairs_in_the_console() {
        let units: Vec<_> = "𠮷".encode_utf16().collect();
        let prefix = [
            ReplayRecord::Release,
            ReplayRecord::Unit(units[0]),
            ReplayRecord::Release,
        ];
        assert_eq!(
            matching_prefix("𠮷", prefix),
            MatchedPrefix {
                records: 1,
                bytes: 0
            }
        );
        assert_eq!(
            matching_prefix(
                "𠮷",
                prefix.into_iter().chain([ReplayRecord::Unit(units[1])])
            ),
            MatchedPrefix {
                records: 4,
                bytes: 4
            }
        );
    }

    /// 【终端】【粘贴回归】制表和换行按剪贴板正文逐个匹配；无参数，返回无
    #[test]
    fn matches_tabs_and_newlines() {
        let text = "甲\t乙\n丙";
        let records = text
            .encode_utf16()
            .flat_map(|unit| [ReplayRecord::Unit(unit), ReplayRecord::Release]);
        assert_eq!(
            matching_prefix(text, records),
            MatchedPrefix {
                records: 9,
                bytes: text.len()
            }
        );
    }
}
