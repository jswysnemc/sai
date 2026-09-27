use super::client::{noul_probabilities, JevClient};
use crate::config::JevConnection;
use serde::Serialize;
use serde_json::{json, Map};
use std::time::Instant;

/// 连接测试超时秒数。
const PROBE_TIMEOUT_SECONDS: u64 = 15;

/// 连接测试结果。
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ProbeReport {
    /// 是否收到有效 Noul 答案
    pub ok: bool,
    /// 请求地址
    pub endpoint: String,
    /// 请求使用的模型
    pub model: String,
    /// 服务端回报的实际模型版本
    pub served_model: Option<String>,
    /// 总耗时毫秒
    pub duration_ms: u128,
    /// 结果说明
    pub detail: String,
}

impl ProbeReport {
    /// 构造未发出请求的失败结果。
    ///
    /// 参数:
    /// - `connection`: 目标接入
    /// - `detail`: 失败原因
    ///
    /// 返回:
    /// - 失败结果
    pub(crate) fn failed(connection: &JevConnection, detail: &str) -> Self {
        Self {
            ok: false,
            endpoint: connection.info.endpoint.clone(),
            model: connection.info.model.clone(),
            served_model: None,
            duration_ms: 0,
            detail: detail.to_string(),
        }
    }

    /// 生成终端展示用的单段摘要。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 包含结果、地址、模型与耗时的文本
    pub(crate) fn summary(&self) -> String {
        let status = if self.ok {
            crate::i18n::text("Jev connection OK", "Jev 连接正常")
        } else {
            crate::i18n::text("Jev connection failed", "Jev 连接失败")
        };
        let model = self.served_model.as_deref().unwrap_or(&self.model);
        format!(
            "{status}: {} · {model} · {} ms\n{}",
            self.endpoint, self.duration_ms, self.detail
        )
    }
}

/// 【Jev接入】【连接测试】发送一个常识 Noul 问题，验证地址、密钥与响应格式。
///
/// 参数:
/// - `connection`: 已解析密钥的接入
///
/// 返回:
/// - 测试结果；失败原因写入 `detail`，不包含密钥
pub(crate) async fn probe(connection: &JevConnection) -> ProbeReport {
    let started = Instant::now();
    let mut report = ProbeReport::failed(connection, "");
    // 1. 构造一个答案明确的问题
    let mut questions = Map::new();
    questions.insert(
        "probe".to_string(),
        json!({
            "type": "noul",
            "instructions": "Is water made of hydrogen and oxygen?",
            "criteria": {"true": "Yes, it is.", "false": "No, it is not."},
        }),
    );
    // 2. 发送请求并检查答案
    let result = match JevClient::new(connection, PROBE_TIMEOUT_SECONDS) {
        Ok(client) => client.evaluate(&json!({}), questions).await,
        Err(error) => Err(error),
    };
    report.duration_ms = started.elapsed().as_millis();
    match result {
        Ok(reply) => match noul_probabilities(&reply.answers).get("probe") {
            Some(probability) => {
                report.ok = true;
                report.served_model = reply.model;
                report.detail = format!("noul={probability:.2}");
            }
            None => report.detail = "response has no noul answer".to_string(),
        },
        Err(error) => report.detail = format!("{error:#}"),
    }
    report
}
