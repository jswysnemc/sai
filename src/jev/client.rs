use crate::config::JevConnection;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::time::Duration;

/// 单个 Noul 问题：只包含指令与是否两侧的判定标准。
#[derive(Debug, Clone)]
pub(crate) struct NoulQuestion {
    /// 问题指令，可为字符串或结构化对象
    pub instructions: Value,
    /// 回答为是时的含义
    pub criteria_true: String,
    /// 回答为否时的含义
    pub criteria_false: String,
}

/// systemone 响应中本模块关心的字段。
#[derive(Debug, Default, Deserialize)]
pub(crate) struct SystemOneReply {
    /// 实际执行的模型版本
    #[serde(default)]
    pub model: Option<String>,
    /// 以问题 id 为键的原始答案
    #[serde(default)]
    pub answers: Map<String, Value>,
}

/// TypeSafe systemone 评估客户端。
pub(crate) struct JevClient {
    http: reqwest::Client,
    endpoint: String,
    api_key: String,
    model: String,
}

impl JevClient {
    /// 按接入构造客户端。
    ///
    /// 参数:
    /// - `connection`: 已解析密钥的 Jev 接入
    /// - `timeout_seconds`: 单次请求超时秒数
    ///
    /// 返回:
    /// - 客户端；HTTP 客户端构造失败时报错
    pub(crate) fn new(connection: &JevConnection, timeout_seconds: u64) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_seconds.max(1)))
            .build()
            .context("failed to build jev http client")?;
        Ok(Self {
            http,
            endpoint: connection.info.endpoint.clone(),
            api_key: connection.api_key.clone(),
            model: connection.info.model.clone(),
        })
    }

    /// 发送一次 systemone 请求，返回原始答案。
    ///
    /// 参数:
    /// - `state`: 所有问题共享的判断依据
    /// - `questions`: 以问题 id 为键、已编码的问题对象
    ///
    /// 返回:
    /// - 响应中的模型版本与原始答案；状态码异常或响应无效时报错
    pub(crate) async fn evaluate(
        &self,
        state: &Value,
        questions: Map<String, Value>,
    ) -> Result<SystemOneReply> {
        // 1. 组装请求体并发送
        let body = serde_json::json!({
            "state": state,
            "model": self.model,
            "questions": Value::Object(questions),
        });
        let response = self
            .http
            .post(&self.endpoint)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .context("jev request failed")?;
        // 2. 检查状态码，错误正文截断后返回
        let status = response.status();
        let text = response
            .text()
            .await
            .context("failed to read jev response")?;
        if !status.is_success() {
            bail!("jev returned HTTP {status}: {}", clip(&text, 300));
        }
        // 3. 解析答案
        serde_json::from_str(&text)
            .with_context(|| format!("invalid jev response: {}", clip(&text, 300)))
    }

    /// 一次请求批量评估多个 Noul 问题。
    ///
    /// 参数:
    /// - `state`: 共享的判断依据
    /// - `questions`: 以问题 id 为键的 Noul 问题
    ///
    /// 返回:
    /// - 问题 id 到"是"概率的映射；缺失答案的问题不出现在结果中
    pub(crate) async fn evaluate_nouls(
        &self,
        state: &Value,
        questions: &BTreeMap<String, NoulQuestion>,
    ) -> Result<BTreeMap<String, f64>> {
        if questions.is_empty() {
            return Ok(BTreeMap::new());
        }
        let reply = self.evaluate(state, encode_nouls(questions)).await?;
        Ok(noul_probabilities(&reply.answers))
    }
}

/// 把 Noul 问题编码为 systemone 问题对象。
///
/// 参数:
/// - `questions`: Noul 问题集合
///
/// 返回:
/// - 以问题 id 为键的问题对象
pub(crate) fn encode_nouls(questions: &BTreeMap<String, NoulQuestion>) -> Map<String, Value> {
    questions
        .iter()
        .map(|(id, question)| {
            (
                id.clone(),
                serde_json::json!({
                    "type": "noul",
                    "instructions": question.instructions,
                    "criteria": {
                        "true": question.criteria_true,
                        "false": question.criteria_false,
                    },
                }),
            )
        })
        .collect()
}

/// 从原始答案中提取 Noul 概率。
///
/// 参数:
/// - `answers`: 以问题 id 为键的原始答案
///
/// 返回:
/// - 问题 id 到概率的映射，概率截断到 [0, 1]，非 Noul 答案被忽略
pub(crate) fn noul_probabilities(answers: &Map<String, Value>) -> BTreeMap<String, f64> {
    answers
        .iter()
        .filter_map(|(id, answer)| {
            answer
                .get("noul")
                .and_then(Value::as_f64)
                .map(|value| (id.clone(), value.clamp(0.0, 1.0)))
        })
        .collect()
}

/// 截断过长文本，供错误信息使用。
pub(crate) fn clip(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let clipped: String = text.chars().take(max_chars).collect();
    format!("{clipped}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noul_questions_are_encoded_with_criteria() {
        let mut questions = BTreeMap::new();
        questions.insert(
            "t0".to_string(),
            NoulQuestion {
                instructions: Value::String("Need it?".to_string()),
                criteria_true: "yes".to_string(),
                criteria_false: "no".to_string(),
            },
        );
        let encoded = encode_nouls(&questions);
        assert_eq!(encoded["t0"]["type"], "noul");
        assert_eq!(encoded["t0"]["criteria"]["false"], "no");
    }

    #[test]
    fn parses_and_clamps_noul_answers() {
        let text = r#"{"model":"jev-1.13.0","answers":{"t0":{"type":"noul","noul":0.8},"t1":{"type":"noul","noul":1.4},"c":{"type":"choice","choice":"x"}},"usage":{"input_tokens":1,"output_tokens":1}}"#;
        let reply: SystemOneReply = serde_json::from_str(text).unwrap();
        let answers = noul_probabilities(&reply.answers);
        assert_eq!(reply.model.as_deref(), Some("jev-1.13.0"));
        assert_eq!(answers.get("t0"), Some(&0.8));
        assert_eq!(answers.get("t1"), Some(&1.0));
        assert!(!answers.contains_key("c"));
    }
}
