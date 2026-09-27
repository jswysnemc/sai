use crate::config::JevRoutingConfig;
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
#[derive(Debug, Deserialize)]
struct SystemOneResponse {
    #[serde(default)]
    answers: BTreeMap<String, SystemOneAnswer>,
}

/// 单个答案；只解析 Noul 概率。
#[derive(Debug, Deserialize)]
struct SystemOneAnswer {
    #[serde(default)]
    noul: Option<f64>,
}

/// TypeSafe systemone 评估客户端。
pub(crate) struct JevClient {
    http: reqwest::Client,
    endpoint: String,
    api_key: String,
    model: String,
}

impl JevClient {
    /// 按配置构造客户端。
    ///
    /// 参数:
    /// - `config`: Jev 暴露决策配置
    ///
    /// 返回:
    /// - 客户端；密钥缺失或 HTTP 客户端构造失败时报错
    pub(crate) fn from_config(config: &JevRoutingConfig) -> Result<Self> {
        let api_key = config.resolved_api_key()?;
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds.max(1)))
            .build()
            .context("failed to build jev http client")?;
        Ok(Self {
            http,
            endpoint: config.endpoint(),
            api_key,
            model: config.model.trim().to_string(),
        })
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
        // 1. 组装请求体
        let body = build_request_body(&self.model, state, questions);
        // 2. 发送请求并检查状态码
        let response = self
            .http
            .post(&self.endpoint)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .context("jev request failed")?;
        let status = response.status();
        let text = response
            .text()
            .await
            .context("failed to read jev response")?;
        if !status.is_success() {
            bail!("jev returned HTTP {status}: {}", clip(&text, 300));
        }
        // 3. 解析 Noul 概率
        parse_noul_answers(&text)
    }
}

/// 组装 systemone 请求体。
///
/// 参数:
/// - `model`: Jev 模型名
/// - `state`: 共享判断依据
/// - `questions`: Noul 问题集合
///
/// 返回:
/// - 可直接序列化的 JSON 请求体
fn build_request_body(
    model: &str,
    state: &Value,
    questions: &BTreeMap<String, NoulQuestion>,
) -> Value {
    let mut encoded = Map::new();
    for (id, question) in questions {
        encoded.insert(
            id.clone(),
            serde_json::json!({
                "type": "noul",
                "instructions": question.instructions,
                "criteria": {
                    "true": question.criteria_true,
                    "false": question.criteria_false,
                },
            }),
        );
    }
    serde_json::json!({
        "state": state,
        "model": model,
        "questions": Value::Object(encoded),
    })
}

/// 从响应文本中提取 Noul 概率。
///
/// 参数:
/// - `text`: systemone 响应 JSON 文本
///
/// 返回:
/// - 问题 id 到概率的映射，概率截断到 [0, 1]
fn parse_noul_answers(text: &str) -> Result<BTreeMap<String, f64>> {
    let parsed: SystemOneResponse = serde_json::from_str(text)
        .with_context(|| format!("invalid jev response: {}", clip(text, 300)))?;
    Ok(parsed
        .answers
        .into_iter()
        .filter_map(|(id, answer)| answer.noul.map(|value| (id, value.clamp(0.0, 1.0))))
        .collect())
}

/// 截断过长文本，供错误信息使用。
fn clip(text: &str, max_chars: usize) -> String {
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
    fn request_body_encodes_noul_questions() {
        let mut questions = BTreeMap::new();
        questions.insert(
            "t0".to_string(),
            NoulQuestion {
                instructions: Value::String("Need it?".to_string()),
                criteria_true: "yes".to_string(),
                criteria_false: "no".to_string(),
            },
        );
        let body = build_request_body("jev-latest", &serde_json::json!({"a": 1}), &questions);
        assert_eq!(body["model"], "jev-latest");
        assert_eq!(body["questions"]["t0"]["type"], "noul");
        assert_eq!(body["questions"]["t0"]["criteria"]["false"], "no");
        assert_eq!(body["state"]["a"], 1);
    }

    #[test]
    fn parses_and_clamps_noul_answers() {
        let text = r#"{"model":"jev-1.13.0","answers":{"t0":{"type":"noul","noul":0.8},"t1":{"type":"noul","noul":1.4},"c":{"type":"choice","choice":"x"}},"usage":{"input_tokens":1,"output_tokens":1}}"#;
        let answers = parse_noul_answers(text).unwrap();
        assert_eq!(answers.get("t0"), Some(&0.8));
        assert_eq!(answers.get("t1"), Some(&1.0));
        assert!(!answers.contains_key("c"));
    }
}
