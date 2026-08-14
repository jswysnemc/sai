/// 从记忆正文里提取关联标识。
///
/// 关联写作 `[[name]]`，允许指向尚不存在的条目：那标记的是"这里还有一条
/// 值得写但没写"的线索，报错或过滤掉都会把这个信号丢掉。
///
/// 参数:
/// - `body`: 记忆正文
///
/// 返回:
/// - 按出现顺序排列且去重的关联标识
pub fn extract_links(body: &str) -> Vec<String> {
    let mut found = Vec::new();
    let bytes: Vec<char> = body.chars().collect();
    let mut index = 0;
    while index + 3 < bytes.len() {
        // 1. 定位开括号，非 [[ 直接前进
        if bytes[index] != '[' || bytes[index + 1] != '[' {
            index += 1;
            continue;
        }
        // 2. 找到配对的闭括号，缺失时视为普通文本
        let Some(close) = find_close(&bytes, index + 2) else {
            index += 1;
            continue;
        };
        let name: String = bytes[index + 2..close].iter().collect();
        let name = name.trim().to_string();
        if !name.is_empty() && !found.contains(&name) {
            found.push(name);
        }
        index = close + 2;
    }
    found
}

/// 从指定位置向后寻找 `]]` 的起始下标。
///
/// 参数:
/// - `chars`: 正文字符序列
/// - `from`: 起始下标
///
/// 返回:
/// - 闭括号起始下标；同一行内未闭合时为 None
fn find_close(chars: &[char], from: usize) -> Option<usize> {
    let mut index = from;
    while index + 1 < chars.len() {
        // 换行意味着这不是一个关联标记，避免跨段误配
        if chars[index] == '\n' {
            return None;
        }
        if chars[index] == ']' && chars[index + 1] == ']' {
            return Some(index);
        }
        index += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证提取出正文里的全部关联。
    #[test]
    fn extracts_every_link_in_order() {
        let links = extract_links("参见 [[zh-writing]] 与 [[no-ai-attribution]]。");

        assert_eq!(links, vec!["zh-writing", "no-ai-attribution"]);
    }

    /// 验证重复关联只保留一条。
    #[test]
    fn duplicate_links_collapse() {
        let links = extract_links("[[a]] 又见 [[a]]");

        assert_eq!(links, vec!["a"]);
    }

    /// 验证未闭合的括号不产生关联。
    #[test]
    fn unclosed_brackets_are_ignored() {
        assert!(extract_links("[[未闭合").is_empty());
    }

    /// 验证关联不跨行匹配。
    ///
    /// 跨行会把两段无关文本之间的一切都吞成一个标识。
    #[test]
    fn links_do_not_span_lines() {
        assert!(extract_links("[[开头\n结尾]]").is_empty());
    }

    /// 验证标识两端空白被去掉。
    #[test]
    fn surrounding_whitespace_is_trimmed() {
        assert_eq!(extract_links("[[  slug  ]]"), vec!["slug"]);
    }

    /// 验证空标识被忽略。
    #[test]
    fn empty_links_are_skipped() {
        assert!(extract_links("[[]] 和 [[   ]]").is_empty());
    }

    /// 验证单层方括号不被当作关联。
    #[test]
    fn single_brackets_are_not_links() {
        assert!(extract_links("这是 [普通链接](http://x) 不是关联").is_empty());
    }

    /// 验证中文标识可以被提取。
    #[test]
    fn chinese_identifiers_are_supported() {
        assert_eq!(extract_links("[[中文标识]]"), vec!["中文标识"]);
    }
}
