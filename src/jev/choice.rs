use anyhow::{bail, Result};
use serde_json::Value;

/// 概率分布之和允许的误差。
const SUM_TOLERANCE: f64 = 0.001;

/// 通过完整性校验的 Choice 答案。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChoiceVerdict {
    /// 选中的选项
    pub choice: String,
    /// 选中选项的概率
    pub probability: f64,
    /// 模型给出的置信度
    pub confidence: f64,
}

/// 校验并解析一个 Choice 答案。
///
/// 选中项必须属于给定选项；概率分布必须覆盖全部选项、没有多余选项、
/// 每项都在 [0, 1] 内、选中项概率最高且总和为 1。
///
/// 参数:
/// - `answer`: systemone 返回的单个原始答案
/// - `labels`: 问题允许的全部选项
///
/// 返回:
/// - 校验通过的答案；任一字段缺失或不一致时报错
pub(crate) fn parse_choice(answer: &Value, labels: &[&str]) -> Result<ChoiceVerdict> {
    // 1. 类型、选中项与置信度
    if answer.get("type").and_then(Value::as_str) != Some("choice") {
        bail!("answer is not a choice");
    }
    let choice = answer
        .get("choice")
        .and_then(Value::as_str)
        .filter(|choice| labels.contains(choice))
        .ok_or_else(|| anyhow::anyhow!("choice is missing or not an allowed label"))?;
    let confidence = answer
        .get("confidence")
        .and_then(Value::as_f64)
        .filter(|value| is_probability(*value))
        .ok_or_else(|| anyhow::anyhow!("confidence is missing or out of range"))?;
    // 2. 概率分布：选项集合必须与问题一致
    let probabilities = answer
        .get("probabilities")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow::anyhow!("probabilities are missing"))?;
    if probabilities.len() != labels.len() {
        bail!("probabilities do not cover exactly the allowed labels");
    }
    let mut sum = 0.0;
    for label in labels {
        let value = probabilities
            .get(*label)
            .and_then(Value::as_f64)
            .filter(|value| is_probability(*value))
            .ok_or_else(|| anyhow::anyhow!("probability for {label} is missing or invalid"))?;
        sum += value;
    }
    // 3. 选中项必须是概率最高的一项，且分布总和为 1
    let probability = probabilities[choice].as_f64().unwrap_or_default();
    if probabilities
        .values()
        .filter_map(Value::as_f64)
        .any(|value| value > probability)
    {
        bail!("chosen label is not the most probable");
    }
    if (sum - 1.0).abs() > SUM_TOLERANCE {
        bail!("probabilities do not sum to 1");
    }
    Ok(ChoiceVerdict {
        choice: choice.to_string(),
        probability,
        confidence,
    })
}

/// 判断数值是否为合法概率。
fn is_probability(value: f64) -> bool {
    (0.0..=1.0).contains(&value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const LABELS: [&str; 3] = ["allow", "deny", "abstain"];

    #[test]
    fn valid_distribution_is_accepted() {
        let answer = json!({"type":"choice","choice":"allow","confidence":0.9,
            "probabilities":{"allow":0.95,"deny":0.03,"abstain":0.02}});
        let verdict = parse_choice(&answer, &LABELS).unwrap();
        assert_eq!(verdict.choice, "allow");
        assert_eq!(verdict.probability, 0.95);
        assert_eq!(verdict.confidence, 0.9);
    }

    #[test]
    fn inconsistent_answers_are_rejected() {
        let cases = [
            json!({"type":"noul","noul":0.9}),
            json!({"type":"choice","choice":"maybe","confidence":0.9,"probabilities":{"allow":1.0,"deny":0.0,"abstain":0.0}}),
            json!({"type":"choice","choice":"allow","confidence":1.2,"probabilities":{"allow":1.0,"deny":0.0,"abstain":0.0}}),
            json!({"type":"choice","choice":"allow","confidence":0.9,"probabilities":{"allow":0.6,"deny":0.4}}),
            json!({"type":"choice","choice":"allow","confidence":0.9,"probabilities":{"allow":0.3,"deny":0.6,"abstain":0.1}}),
            json!({"type":"choice","choice":"allow","confidence":0.9,"probabilities":{"allow":0.6,"deny":0.3,"abstain":0.3}}),
            json!({"type":"choice","choice":"allow","confidence":0.9,"probabilities":{"allow":0.6,"deny":0.3,"other":0.1}}),
        ];
        for answer in cases {
            assert!(parse_choice(&answer, &LABELS).is_err(), "{answer}");
        }
    }
}
