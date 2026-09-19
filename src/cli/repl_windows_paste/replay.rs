/// 【终端】【粘贴回放】仅保留批量匹配所需的原生事件语义
#[derive(Clone, Copy, Debug)]
pub(super) enum ReplayRecord {
    Release,
    Unit(u16),
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
pub(super) fn matching_prefix(
    text: &str,
    records: impl IntoIterator<Item = ReplayRecord>,
) -> MatchedPrefix {
    let mut matched = MatchedPrefix::default();
    let mut low_surrogate = false;
    for (index, record) in records.into_iter().enumerate() {
        let Some(expected) = text[matched.bytes..].chars().next() else {
            break;
        };
        let unit = match record {
            ReplayRecord::Boundary => break,
            ReplayRecord::Release => {
                if !low_surrogate {
                    matched.records = index + 1;
                }
                continue;
            }
            ReplayRecord::Unit(unit) => unit,
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
        matched.bytes += expected.len_utf8();
        matched.records = index + 1;
    }
    matched
}

#[cfg(test)]
mod tests {
    use super::*;

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
