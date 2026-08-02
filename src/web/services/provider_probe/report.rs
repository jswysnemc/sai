use serde::Serialize;

/// 单个探测阶段的结果。
#[derive(Debug, Clone, Serialize)]
pub struct ProbeStageResult {
    /// 阶段标识：catalog、completion 或 tool_call
    pub stage: &'static str,
    pub ok: bool,
    pub duration_ms: u64,
    /// 成功时的摘要，或失败时的原始错误信息
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<&'static str>,
}

impl ProbeStageResult {
    /// 构造一个成功的阶段结果。
    ///
    /// 参数:
    /// - `stage`: 阶段标识
    /// - `duration_ms`: 阶段耗时
    /// - `detail`: 成功摘要
    ///
    /// 返回:
    /// - 阶段结果
    pub fn success(stage: &'static str, duration_ms: u64, detail: String) -> Self {
        Self {
            stage,
            ok: true,
            duration_ms,
            detail,
            error_kind: None,
        }
    }

    /// 构造一个失败的阶段结果。
    ///
    /// 参数:
    /// - `stage`: 阶段标识
    /// - `duration_ms`: 阶段耗时
    /// - `error`: 原始错误
    ///
    /// 返回:
    /// - 带失败归类的阶段结果
    pub fn failure(stage: &'static str, duration_ms: u64, error: &anyhow::Error) -> Self {
        let detail = format!("{error:#}");
        let kind = super::error_kind::classify(&detail);
        Self {
            stage,
            ok: false,
            duration_ms,
            detail,
            error_kind: Some(kind.as_str()),
        }
    }
}

/// 一次连通性探测的完整报告。
#[derive(Debug, Clone, Serialize)]
pub struct ProviderProbeReport {
    pub ok: bool,
    pub provider_id: String,
    pub model: String,
    pub total_ms: u64,
    pub stages: Vec<ProbeStageResult>,
    /// 首个失败阶段的归类，全部通过时为空
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<&'static str>,
    /// 探测请求实际消耗的令牌，供应商未上报时为空
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<u64>,
}

impl ProviderProbeReport {
    /// 由各阶段结果汇总出报告。
    ///
    /// 参数:
    /// - `provider_id`: 供应商标识
    /// - `model`: 被探测的模型
    /// - `stages`: 各阶段结果
    /// - `tokens`: 探测消耗的令牌
    ///
    /// 返回:
    /// - 完整报告；任一阶段失败即整体失败
    pub fn from_stages(
        provider_id: String,
        model: String,
        stages: Vec<ProbeStageResult>,
        tokens: Option<u64>,
    ) -> Self {
        let total_ms = stages.iter().map(|stage| stage.duration_ms).sum();
        let failed = stages.iter().find(|stage| !stage.ok);
        Self {
            ok: failed.is_none(),
            provider_id,
            model,
            total_ms,
            error_kind: failed.and_then(|stage| stage.error_kind),
            stages,
            tokens,
        }
    }
}
