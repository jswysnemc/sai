/// 索引里的一条指针。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexEntry {
    /// 展示标题
    pub title: String,
    /// 记忆文件名，含扩展名
    pub file: String,
    /// 一句话提示，供召回时判断是否值得展开正文
    pub hook: String,
}

impl IndexEntry {
    /// 渲染成索引里的一行。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 索引行文本
    pub fn render(&self) -> String {
        if self.hook.trim().is_empty() {
            format!("- [{}]({})", self.title, self.file)
        } else {
            format!("- [{}]({}) — {}", self.title, self.file, self.hook)
        }
    }
}

/// 索引文件的一行。
#[derive(Debug, Clone, PartialEq, Eq)]
enum Line {
    /// 一条记忆指针
    Entry(IndexEntry),
    /// 标题、空行或用户自己写的说明
    Other(String),
}

/// 一份索引文件。
///
/// 非条目行原样保留：索引是给人读也给人改的，重建整个文件会把用户
/// 自己加的标题和说明抹掉。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDocument {
    lines: Vec<Line>,
}

impl IndexDocument {
    /// 解析索引文件内容。
    ///
    /// 参数:
    /// - `content`: 索引全文
    ///
    /// 返回:
    /// - 解析结果
    pub fn parse(content: &str) -> Self {
        Self {
            lines: content
                .lines()
                .map(|line| match parse_entry(line) {
                    Some(entry) => Line::Entry(entry),
                    None => Line::Other(line.to_string()),
                })
                .collect(),
        }
    }

    /// 返回全部指针。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 按出现顺序排列的指针
    pub fn entries(&self) -> Vec<&IndexEntry> {
        self.lines
            .iter()
            .filter_map(|line| match line {
                Line::Entry(entry) => Some(entry),
                Line::Other(_) => None,
            })
            .collect()
    }

    /// 写入或更新一条指针。
    ///
    /// 参数:
    /// - `entry`: 待写入的指针
    ///
    /// 返回:
    /// - 无
    pub fn upsert(&mut self, entry: IndexEntry) {
        // 1. 同一文件已有指针时就地替换，保持原有顺序
        if let Some(slot) = self.lines.iter_mut().find(|line| match line {
            Line::Entry(existing) => existing.file == entry.file,
            Line::Other(_) => false,
        }) {
            *slot = Line::Entry(entry);
            return;
        }
        // 2. 否则插在最后一条指针之后，让指针保持连续成块
        match self
            .lines
            .iter()
            .rposition(|line| matches!(line, Line::Entry(_)))
        {
            Some(last) => self.lines.insert(last + 1, Line::Entry(entry)),
            None => self.lines.push(Line::Entry(entry)),
        }
    }

    /// 移除指向某个文件的指针。
    ///
    /// 参数:
    /// - `file`: 记忆文件名
    ///
    /// 返回:
    /// - 是否确实移除了一条
    pub fn remove(&mut self, file: &str) -> bool {
        let before = self.lines.len();
        self.lines.retain(|line| match line {
            Line::Entry(entry) => entry.file != file,
            Line::Other(_) => true,
        });
        self.lines.len() != before
    }

    /// 渲染成完整文件内容。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 索引全文
    pub fn render(&self) -> String {
        let body = self
            .lines
            .iter()
            .map(|line| match line {
                Line::Entry(entry) => entry.render(),
                Line::Other(text) => text.clone(),
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!("{}\n", body.trim_end())
    }
}

/// 解析一行索引指针。
///
/// 参数:
/// - `line`: 索引行
///
/// 返回:
/// - 指针；该行不是指针时为 None
fn parse_entry(line: &str) -> Option<IndexEntry> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix("- [")?;
    let title_end = rest.find("](")?;
    let title = rest[..title_end].to_string();
    let after = &rest[title_end + 2..];
    let file_end = after.find(')')?;
    let file = after[..file_end].trim().to_string();
    if file.is_empty() {
        return None;
    }
    let hook = after[file_end + 1..]
        .trim_start()
        .trim_start_matches(['—', '-', '–'])
        .trim()
        .to_string();
    Some(IndexEntry { title, file, hook })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一条指针。
    ///
    /// 参数:
    /// - `file`: 文件名
    ///
    /// 返回:
    /// - 指针
    fn entry(file: &str) -> IndexEntry {
        IndexEntry {
            title: "标题".to_string(),
            file: file.to_string(),
            hook: "提示".to_string(),
        }
    }

    /// 验证一行指针可以往返解析。
    #[test]
    fn an_entry_round_trips() {
        let rendered = entry("a.md").render();

        assert_eq!(parse_entry(&rendered), Some(entry("a.md")));
    }

    /// 验证非指针行被原样保留。
    ///
    /// 用户可能在索引里写了标题和说明，重建会把它们抹掉。
    #[test]
    fn non_entry_lines_are_preserved() {
        let content = "# 记忆索引\n\n- [标题](a.md) — 提示\n\n末尾说明";

        let document = IndexDocument::parse(content);

        assert!(document.render().contains("# 记忆索引"));
        assert!(document.render().contains("末尾说明"));
    }

    /// 验证同一文件的指针被替换而不是追加。
    #[test]
    fn upsert_replaces_an_existing_pointer() {
        let mut document = IndexDocument::parse("- [旧](a.md) — 旧提示");

        document.upsert(entry("a.md"));

        assert_eq!(document.entries().len(), 1);
        assert_eq!(document.entries()[0].title, "标题");
    }

    /// 验证新指针插在指针块末尾而不是文件末尾。
    ///
    /// 追加到文件末尾会把它插到用户写的说明之后。
    #[test]
    fn a_new_pointer_joins_the_existing_block() {
        let mut document = IndexDocument::parse("- [一](a.md)\n\n末尾说明");

        document.upsert(entry("b.md"));

        let rendered = document.render();
        let pointer_at = rendered.find("b.md").unwrap();
        let note_at = rendered.find("末尾说明").unwrap();
        assert!(pointer_at < note_at);
    }

    /// 验证移除只影响目标指针。
    #[test]
    fn removing_touches_only_the_target() {
        let mut document = IndexDocument::parse("- [一](a.md)\n- [二](b.md)");

        assert!(document.remove("a.md"));

        assert_eq!(document.entries().len(), 1);
        assert_eq!(document.entries()[0].file, "b.md");
    }

    /// 验证移除不存在的指针不报成功。
    #[test]
    fn removing_a_missing_pointer_reports_false() {
        let mut document = IndexDocument::parse("- [一](a.md)");

        assert!(!document.remove("zzz.md"));
    }

    /// 验证没有提示的指针也能解析。
    #[test]
    fn a_pointer_without_a_hook_is_valid() {
        let parsed = parse_entry("- [标题](a.md)").unwrap();

        assert_eq!(parsed.hook, "");
    }

    /// 验证普通 markdown 链接不被当成指针。
    #[test]
    fn an_ordinary_link_is_not_a_pointer() {
        assert!(parse_entry("参见 [文档](http://x)").is_none());
    }

    /// 验证空索引渲染后不产生孤立空行。
    #[test]
    fn an_empty_index_renders_cleanly() {
        assert_eq!(IndexDocument::parse("").render(), "\n");
    }
}
