use super::*;
use crate::config_tui::provider_fetch::{fetch_models, FetchModelsResult};

/// 【供应商配置】【目录更新】计算影响模型请求的接入指纹，不保存或输出凭据明文。
/// 参数: provider 为供应商草稿；返回: 接入配置摘要
pub(super) fn connection_key(provider: &ProviderConfig) -> String {
    let value = serde_json::to_vec(&(
        &provider.id,
        &provider.base_url,
        &provider.protocol,
        provider.enabled,
        &provider.api_key,
        &provider.api_keys,
        &provider.api_key_selected,
        provider.api_key_balance,
        &provider.extra_headers,
    ))
    .expect("provider connection is serializable");
    blake3::hash(&value).to_hex().to_string()
}

/// 【供应商配置】【目录导入】可取消地获取并合并模型，失败时保留本地配置。
/// 参数: stdout 为终端，provider 为草稿，notify_success 指定成功后是否停留提示
/// 返回: 成功为 Some(true)，请求失败为 Some(false)，取消为 None
pub(super) fn refresh(
    stdout: &mut io::Stdout,
    provider: &mut ProviderConfig,
    notify_success: bool,
) -> Result<Option<bool>> {
    let draft = provider.clone();
    let Some(result) = crate::config_tui::background::run(
        stdout,
        t(" MODEL CONNECTION TEST ", " 模型连接测试 "),
        move || fetch_models(&draft),
    )?
    else {
        return Ok(None);
    };
    match result {
        Ok(result) => {
            merge(provider, result);
            if notify_success {
                message(
                    stdout,
                    t(
                        "Connection succeeded; models imported",
                        "连接成功，已导入模型",
                    ),
                )?;
            }
            Ok(Some(true))
        }
        Err(error) => {
            message(
                stdout,
                &format!("{}\n\n{error}", t(
                "Could not import models. Local models are kept; add models manually if needed.",
                "无法导入模型。已保留本地模型，可在模型列表中手动添加。",
            )),
            )?;
            Ok(Some(false))
        }
    }
}

/// 【供应商配置】【目录合并】导入可选模型和缺失元数据，保持已配置的默认模型。
/// 参数: provider 为草稿，result 为远端目录；返回: 无
fn merge(provider: &mut ProviderConfig, result: FetchModelsResult) {
    // 1. 导入模型并去重，确保 /model 使用的持久化列表包含远端结果
    for model in result.models {
        if !provider.models.contains(&model) {
            provider.models.push(model);
        }
    }
    // 2. 手动配置的元数据优先，仅补充未配置的模型
    for (model, metadata) in result.metadata {
        provider.model_metadata.entry(model).or_insert(metadata);
    }
    provider.infer_default_model();
}
