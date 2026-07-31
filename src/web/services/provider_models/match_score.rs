//! 模型 ID 匹配与官方供应商标识排序。

/// 生成用于跨目录匹配的模型 ID 候选。
///
/// 参数:
/// - `model`: 本地模型标识
///
/// 返回:
/// - 去重后的小写候选 ID
pub(super) fn model_id_candidates(model: &str) -> Vec<String> {
    let trimmed = model.trim().to_ascii_lowercase();
    let mut candidates = vec![trimmed.clone()];
    if let Some((_, bare)) = trimmed.split_once('/') {
        if !bare.is_empty() {
            candidates.push(bare.to_string());
        }
    }
    if let Some((bare, _)) = trimmed.rsplit_once(':') {
        if !bare.is_empty() {
            candidates.push(bare.to_string());
        }
    }
    candidates.sort();
    candidates.dedup();
    candidates
}

/// 计算本地模型 ID 与远程 ID 的匹配分数，越高越优先。
///
/// 参数:
/// - `candidates`: 本地模型候选 ID
/// - `remote`: 远程目录模型 ID
///
/// 返回:
/// - 匹配分数；无匹配时返回 None
pub(super) fn match_score(candidates: &[String], remote: &str) -> Option<usize> {
    let remote = remote.trim().to_ascii_lowercase();
    if remote.is_empty() {
        return None;
    }
    let remote_bare = remote
        .rsplit_once('/')
        .map(|(_, bare)| bare)
        .unwrap_or(remote.as_str());
    let mut best = 0usize;
    for candidate in candidates {
        if candidate == &remote {
            best = best.max(300);
            continue;
        }
        if candidate == remote_bare {
            best = best.max(250);
            continue;
        }
        if remote.ends_with(&format!("/{candidate}")) {
            best = best.max(220);
            continue;
        }
        if candidate.ends_with(&format!("/{remote_bare}")) {
            best = best.max(200);
            continue;
        }
        if (remote_bare.starts_with(candidate) || candidate.starts_with(remote_bare))
            && candidate.len() >= 8
            && remote_bare.len() >= 8
        {
            best = best.max(120);
        }
    }
    (best > 0).then_some(best)
}

/// 在相同匹配分数下，优先官方模型族供应商标识，便于前端图标映射。
///
/// 参数:
/// - `score`: 模型 ID 匹配分数
/// - `provider`: 目录返回的供应商标识
///
/// 返回:
/// - 用于比较的综合排序分
pub(super) fn rank_catalog_match(score: usize, provider: &str) -> usize {
    score.saturating_mul(10) + official_provider_bonus(provider)
}

/// 返回官方或常见模型族供应商标识的加分。
///
/// 参数:
/// - `provider`: 供应商标识
///
/// 返回:
/// - 0-9 的偏好加分
fn official_provider_bonus(provider: &str) -> usize {
    let provider = provider.trim().to_ascii_lowercase();
    match provider.as_str() {
        "openai" | "anthropic" | "google" | "google-vertex" | "deepseek" | "alibaba"
        | "alibaba-cn" | "qwen" | "dashscope" | "zhipuai" | "moonshotai" | "moonshot"
        | "mistral" | "meta" | "xai" | "cohere" | "perplexity" | "minimax" | "bytedance"
        | "tencent" | "baidu" | "stepfun" | "groq" | "togetherai" | "fireworks-ai" => 9,
        // 阿里系编码/Token 计划与官方标识相近，保留较高优先级
        "alibaba-token-plan"
        | "alibaba-token-plan-cn"
        | "alibaba-coding-plan"
        | "alibaba-coding-plan-cn" => 7,
        // 聚合转发平台优先级低于官方模型族
        "openrouter" | "llmgateway" | "helicone" | "nano-gpt" | "poe" | "cloudflare-ai-gateway" => {
            2
        }
        _ => 4,
    }
}
