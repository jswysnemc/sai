mod error_kind;
mod report;

pub use report::{ProbeStageResult, ProviderProbeReport};

use crate::config::{AppConfig, ProviderConfig};
use crate::llm::{ChatMessage, OpenAiCompatibleClient};
use crate::paths::SaiPaths;
use anyhow::Result;
use std::time::Instant;

/// 探测请求发送的最短提示词。
const PROBE_PROMPT: &str = "ping";

/// 探测允许的最大输出长度，够判断链路通即可。
const PROBE_MAX_TOKENS: u32 = 16;

/// 探测的整体超时上限（秒）。
const PROBE_TIMEOUT_SECONDS: u64 = 20;

/// 对一个供应商执行连通性探测。
///
/// 分两个阶段，任一阶段失败即停止：
/// 1. catalog：拉取模型列表，验证地址可达与凭据有效
/// 2. completion：发一条最小对话，验证该模型标识真能出结果
///
/// 分阶段是为了区分"连不上""密钥不对""模型名写错"这三类问题——
/// 只发一次对话请求的话，三者返回的错误往往难以分辨。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 当前应用配置
/// - `provider`: 待探测的供应商配置（密钥须已还原）
/// - `model`: 待探测的模型标识；为空时取供应商默认模型
///
/// 返回:
/// - 探测报告；网络异常不会返回 Err，而是记入报告
pub async fn probe_provider(
    paths: &SaiPaths,
    config: &AppConfig,
    provider: &ProviderConfig,
    model: Option<&str>,
) -> Result<ProviderProbeReport> {
    let target_model = resolve_model(provider, model);
    let mut stages = Vec::new();

    // 1. 目录阶段：地址与凭据
    let catalog_started = Instant::now();
    let catalog = fetch_catalog(paths, provider).await;
    let catalog_ms = catalog_started.elapsed().as_millis() as u64;
    match &catalog {
        Ok(models) => stages.push(ProbeStageResult::success(
            "catalog",
            catalog_ms,
            catalog_detail(models, &target_model),
        )),
        Err(error) => {
            stages.push(ProbeStageResult::failure("catalog", catalog_ms, error));
            return Ok(ProviderProbeReport::from_stages(
                provider.id.clone(),
                target_model,
                stages,
                None,
            ));
        }
    }

    // 2. 推理阶段：该模型是否真的可用
    let completion_started = Instant::now();
    let completion = run_completion(paths, config, provider, &target_model).await;
    let completion_ms = completion_started.elapsed().as_millis() as u64;
    let tokens = match &completion {
        Ok(outcome) => {
            stages.push(ProbeStageResult::success(
                "completion",
                completion_ms,
                outcome.summary.clone(),
            ));
            outcome.tokens
        }
        Err(error) => {
            stages.push(ProbeStageResult::failure(
                "completion",
                completion_ms,
                error,
            ));
            None
        }
    };

    Ok(ProviderProbeReport::from_stages(
        provider.id.clone(),
        target_model,
        stages,
        tokens,
    ))
}

/// 推理阶段的产出。
struct CompletionOutcome {
    summary: String,
    tokens: Option<u64>,
}

/// 选定探测目标模型。
///
/// 参数:
/// - `provider`: 供应商配置
/// - `model`: 显式指定的模型
///
/// 返回:
/// - 显式模型；未指定时依次回落到默认模型与模型列表首项
fn resolve_model(provider: &ProviderConfig, model: Option<&str>) -> String {
    let explicit = model.map(str::trim).filter(|value| !value.is_empty());
    if let Some(value) = explicit {
        return value.to_string();
    }
    if !provider.default_model.trim().is_empty() {
        return provider.default_model.clone();
    }
    provider.models.first().cloned().unwrap_or_default()
}

/// 拉取模型目录。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `provider`: 供应商配置
///
/// 返回:
/// - 模型标识列表
async fn fetch_catalog(paths: &SaiPaths, provider: &ProviderConfig) -> Result<Vec<String>> {
    let paths = paths.clone();
    let provider = provider.clone();
    tokio::task::spawn_blocking(move || {
        super::provider_models::fetch_models(&paths, &provider).map(|result| result.models)
    })
    .await?
}

/// 组装目录阶段的成功摘要。
///
/// 参数:
/// - `models`: 拉到的模型列表
/// - `target`: 待探测模型
///
/// 返回:
/// - 含模型总数的摘要；目标模型不在列表中时额外提示
fn catalog_detail(models: &[String], target: &str) -> String {
    let count = models.len();
    if target.is_empty() || models.iter().any(|model| model == target) {
        return format!("{count} models");
    }
    // 不少中转站的目录接口并不完整，这里只提示而不判定为失败
    format!("{count} models; \"{target}\" not listed")
}

/// 发送一次最小对话请求。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 当前应用配置
/// - `provider`: 供应商配置
/// - `model`: 目标模型
///
/// 返回:
/// - 响应摘要与消耗令牌
async fn run_completion(
    paths: &SaiPaths,
    config: &AppConfig,
    provider: &ProviderConfig,
    model: &str,
) -> Result<CompletionOutcome> {
    // 用一份收紧过的配置副本，避免探测请求受用户的长超时与大 max_tokens 影响
    let mut probe_provider = provider.clone();
    probe_provider.default_model = model.to_string();
    probe_provider.timeout_seconds = probe_provider.timeout_seconds.min(PROBE_TIMEOUT_SECONDS);
    probe_provider.anthropic_max_tokens = probe_provider.anthropic_max_tokens.min(PROBE_MAX_TOKENS);
    let client = OpenAiCompatibleClient::new(&probe_provider, config, paths)?;
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(PROBE_TIMEOUT_SECONDS),
        client.chat_stream(
            vec![ChatMessage::plain("user", PROBE_PROMPT)],
            Vec::new(),
            |_| Ok(()),
        ),
    )
    .await
    .map_err(|_| anyhow::anyhow!("probe timed out after {PROBE_TIMEOUT_SECONDS}s"))??;
    let tokens = result.usage.as_ref().map(|usage| usage.total_tokens);
    Ok(CompletionOutcome {
        summary: summarize_reply(&result.content),
        tokens,
    })
}

/// 把模型回复压成一行摘要。
///
/// 参数:
/// - `content`: 模型回复全文
///
/// 返回:
/// - 去除换行并截断后的摘要；空回复时给出占位说明
fn summarize_reply(content: &str) -> String {
    let flattened = content.split_whitespace().collect::<Vec<_>>().join(" ");
    if flattened.is_empty() {
        return "empty reply".to_string();
    }
    let truncated: String = flattened.chars().take(60).collect();
    if truncated.chars().count() < flattened.chars().count() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider_with(default_model: &str, models: &[&str]) -> ProviderConfig {
        ProviderConfig {
            id: "p".to_string(),
            display_name: "p".to_string(),
            base_url: "https://example.test/v1".to_string(),
            protocol: "auto".to_string(),
            api_key: None,
            models: models.iter().map(|model| model.to_string()).collect(),
            model_context_chars: std::collections::HashMap::new(),
            model_metadata: std::collections::HashMap::new(),
            default_model: default_model.to_string(),
            timeout_seconds: 60,
            temperature: 0.7,
            anthropic_max_tokens: 4096,
            thinking_level: "auto".to_string(),
            thinking_format: "auto".to_string(),
            extra_body: String::new(),
            extra_headers: std::collections::HashMap::new(),
            user_agent: String::new(),
            client_style: "auto".to_string(),
            claude_1m_context: true,
        }
    }

    /// 验证目标模型的取值优先级。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无；断言失败则测试不通过
    #[test]
    fn resolves_target_model_by_priority() {
        let provider = provider_with("default-model", &["first-model", "other"]);
        assert_eq!(resolve_model(&provider, Some("explicit")), "explicit");
        assert_eq!(resolve_model(&provider, Some("  ")), "default-model");
        assert_eq!(resolve_model(&provider, None), "default-model");
        let without_default = provider_with("", &["first-model"]);
        assert_eq!(resolve_model(&without_default, None), "first-model");
    }

    /// 验证目录摘要在模型缺席时给出提示。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无；断言失败则测试不通过
    #[test]
    fn flags_model_missing_from_catalog() {
        let models = vec!["a".to_string(), "b".to_string()];
        assert_eq!(catalog_detail(&models, "a"), "2 models");
        assert_eq!(catalog_detail(&models, "z"), "2 models; \"z\" not listed");
    }

    /// 验证回复摘要压平换行并在超长时截断。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无；断言失败则测试不通过
    #[test]
    fn summarizes_reply_into_single_line() {
        assert_eq!(summarize_reply("hello\n  world "), "hello world");
        assert_eq!(summarize_reply("   "), "empty reply");
        let long = "x".repeat(80);
        let summary = summarize_reply(&long);
        assert!(summary.ends_with('…'));
        assert_eq!(summary.chars().count(), 61);
    }
}
