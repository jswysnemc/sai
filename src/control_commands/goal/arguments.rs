use anyhow::{bail, Result};

/// 【目标】【参数解析】提取预算选项，保持需求正文的换行、缩进与空白。
/// 参数: `input` 为目标与预算参数；返回: 完整目标和可选预算，非法参数返回错误。
pub(super) fn parse_objective_and_budget(input: &str) -> Result<(String, Option<u64>)> {
    let mut parts = input.split_whitespace();
    let mut objective = String::new();
    let mut token_budget = None;
    let mut cursor = 0;
    let mut search_start = 0;
    while let Some(part) = parts.next() {
        // 1. 【目标】【参数解析】按原文位置删除选项，不重新拼接正文词语
        let start = search_start + input[search_start..].find(part).unwrap();
        search_start = start + part.len();
        let value = if part == "--tokens" {
            let value = parts
                .next()
                .ok_or_else(|| anyhow::anyhow!("--tokens requires a positive integer"))?;
            search_start += input[search_start..].find(value).unwrap() + value.len();
            Some(value)
        } else {
            part.strip_prefix("--tokens=")
        };
        if let Some(value) = value {
            token_budget = Some(parse_budget(value)?);
            objective.push_str(&input[cursor..start]);
            cursor = search_start;
        }
    }
    // 2. 【目标】【参数解析】仅去除目标首尾空白，正文格式保持不变
    objective.push_str(&input[cursor..]);
    let objective = objective.trim().to_string();
    if objective.is_empty() {
        bail!("goal objective cannot be empty")
    }
    Ok((objective, token_budget))
}

/// 【目标】【参数解析】校验正整数预算。
/// 参数: `value` 为预算文本；返回: 正整数预算或参数错误。
fn parse_budget(value: &str) -> Result<u64> {
    value
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| anyhow::anyhow!("--tokens requires a positive integer"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【目标】【参数测试】多行正文与代码缩进不随预算解析改变。
    /// 参数: 无；返回: 无，格式不一致时断言失败。
    #[test]
    fn preserves_objective_layout_with_optional_budget() {
        let body = "修复目标\n\n```rust\n    let value =  2;\n```\n完成测试";
        for (input, budget) in [
            (body.to_string(), None),
            (format!("{body} --tokens 4000"), Some(4000)),
            (format!("--tokens=4000\n{body}"), Some(4000)),
        ] {
            assert_eq!(
                parse_objective_and_budget(&input).unwrap(),
                (body.into(), budget)
            );
        }
    }

    /// 【目标】【参数测试】缺失目标或非法预算不创建目标。
    /// 参数: 无；返回: 无，接受非法输入时断言失败。
    #[test]
    fn rejects_invalid_budget_and_empty_objective() {
        for input in [
            "",
            "--tokens 42",
            "task --tokens",
            "task --tokens=0",
            "task --tokens -1",
            "task --tokens invalid",
        ] {
            assert!(parse_objective_and_budget(input).is_err(), "{input}");
        }
    }
}
