use crate::render::terminal_image::escape_sequence_end;
use crate::render::transcript::AnsiLine;

/// 命中文字在原始 ANSI 行中的字节范围
#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct SourceSpan {
    pub(super) row: usize,
    pub(super) start: usize,
    pub(super) end: usize,
}

/// 一个归一化字符与原始文本的对应关系
#[derive(Debug, Clone, Eq, PartialEq)]
struct IndexedSpan {
    end: usize,
    source: SourceSpan,
}

/// 【终端】【历史搜索】复用可见文本索引；搜索词变化不重复解析 ANSI
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub(super) struct SearchIndex {
    lines: Vec<AnsiLine>,
    text: String,
    spans: Vec<IndexedSpan>,
}

impl SearchIndex {
    /// 内容变化时重建索引，lines 为已经折行的正文；返回是否重建
    pub(super) fn refresh(&mut self, lines: &[AnsiLine]) -> bool {
        if self.lines == lines {
            return false;
        }
        self.text.clear();
        self.spans.clear();
        for (row, line) in lines.iter().enumerate() {
            let text = line.as_str();
            let mut offset = 0;
            while offset < text.len() {
                if text.as_bytes()[offset] == 0x1b {
                    offset = escape_sequence_end(text, offset);
                    continue;
                }
                let ch = text[offset..].chars().next().expect("character boundary");
                let end = offset + ch.len_utf8();
                self.push(
                    ch,
                    SourceSpan {
                        row,
                        start: offset,
                        end,
                    },
                );
                offset = end;
            }
            self.push(
                ' ',
                SourceSpan {
                    row,
                    start: text.len(),
                    end: text.len(),
                },
            );
        }
        self.lines = lines.to_vec();
        true
    }

    /// 追加一个归一化字符及其位置，ch 为字符，source 为原始范围；返回无
    fn push(&mut self, ch: char, source: SourceSpan) {
        if ch.is_whitespace() {
            if self.text.ends_with(' ') {
                // 1. 【终端】【历史搜索】连续空白只占一个搜索字符
                if let Some(last) = self.spans.last_mut() {
                    if last.source.row == source.row {
                        last.source.end = source.end;
                    }
                }
                return;
            }
            self.text.push(' ');
        } else if ch.is_control() {
            return;
        } else {
            self.text.extend(ch.to_lowercase());
        }
        self.spans.push(IndexedSpan {
            end: self.text.len(),
            source,
        });
    }

    /// 查找 query 对应的全部命中；返回每次命中的跨行范围
    pub(super) fn find(&self, query: &str) -> Vec<Vec<SourceSpan>> {
        let query = query
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        if query.is_empty() {
            return Vec::new();
        }
        self.text
            .match_indices(&query)
            .map(|(start, _)| {
                let end = start + query.len();
                let first = self.spans.partition_point(|span| span.end <= start);
                let mut result: Vec<SourceSpan> = Vec::new();
                for span in &self.spans[first..] {
                    if span.source.start != span.source.end {
                        match result.last_mut() {
                            Some(last) if last.row == span.source.row => last.end = span.source.end,
                            _ => result.push(span.source.clone()),
                        }
                    }
                    if span.end >= end {
                        break;
                    }
                }
                result
            })
            .filter(|spans| !spans.is_empty())
            .collect()
    }
}
