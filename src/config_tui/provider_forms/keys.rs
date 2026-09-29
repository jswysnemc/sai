/// 把多密钥列表渲染成表单可编辑的多行文本。
///
/// 每行一个密钥，备注非空时以 ` | ` 分隔附在密钥之后。
///
/// 参数:
/// - `keys`: 多密钥列表
///
/// 返回:
/// - 多行文本；空列表返回空串
pub(crate) fn render_api_key_lines(keys: &[crate::config::ProviderApiKey]) -> String {
    keys.iter()
        .map(|key| {
            if key.label.is_empty() {
                key.api_key.clone()
            } else {
                format!("{} | {}", key.api_key, key.label)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 解析多行文本为多密钥列表，按密钥内容复用原标识以保持脱敏对齐。
///
/// 未在原列表出现的密钥分配新标识；空行被忽略。
///
/// 参数:
/// - `text`: 表单多行文本
/// - `original`: 原多密钥列表，用于按内容复用标识
///
/// 返回:
/// - 解析后的多密钥列表
pub(crate) fn parse_api_key_lines(
    text: &str,
    original: &[crate::config::ProviderApiKey],
) -> Vec<crate::config::ProviderApiKey> {
    let used: std::collections::HashSet<&str> =
        original.iter().map(|key| key.id.as_str()).collect();
    let mut next_id = 1usize;
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            // 1. 以首个 ` | ` 切分密钥与备注，密钥本身可能含竖线故只切第一段
            let (value, label) = match line.split_once(" | ") {
                Some((value, label)) => (value.trim().to_string(), label.trim().to_string()),
                None => (line.to_string(), String::new()),
            };
            // 2. 内容未变则复用原标识，保证与 Web 脱敏回填对齐
            let id = original
                .iter()
                .find(|key| key.api_key == value)
                .map(|key| key.id.clone())
                .unwrap_or_else(|| {
                    while used.contains(format!("key-{next_id}").as_str()) {
                        next_id += 1;
                    }
                    let fresh = format!("key-{next_id}");
                    next_id += 1;
                    fresh
                });
            crate::config::ProviderApiKey {
                id,
                api_key: value,
                label,
            }
        })
        .collect()
}

/// 解析 1 基序号为对应的密钥标识。
///
/// 越界或非数字时返回空，表示回落到首个密钥。
///
/// 参数:
/// - `text`: 表单序号文本
/// - `keys`: 多密钥列表
///
/// 返回:
/// - 命中时返回密钥标识，否则 None
pub(crate) fn parse_selected_key(
    text: &str,
    keys: &[crate::config::ProviderApiKey],
) -> Option<String> {
    let index: usize = text.trim().parse().ok()?;
    keys.get(index.checked_sub(1)?).map(|key| key.id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【模型接入】【密钥回归】重排和编辑备注后仍保留原有密钥标识。
    /// 参数: 无；返回: 无
    #[test]
    fn key_identity_survives_reordering() {
        let original = parse_api_key_lines("fixture-a | first\nfixture-b | second", &[]);
        let parsed = parse_api_key_lines("fixture-b | new label\nfixture-a | first", &original);
        assert_eq!(parsed[0].id, original[1].id);
        assert_eq!(parsed[1].id, original[0].id);
        assert_eq!(
            parse_selected_key("2", &parsed),
            Some(original[0].id.clone())
        );
        assert_eq!(
            render_api_key_lines(&original),
            "fixture-a | first\nfixture-b | second"
        );
    }
}
