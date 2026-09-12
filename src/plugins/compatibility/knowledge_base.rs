use super::RuntimeOverrides;
use crate::{config::AppConfig, paths::SaiPaths};
use anyhow::{Context, Result};
use sai_plugin_runtime::Capabilities;
use serde_json::{json, Value};

/// 【知识库兼容】【设置快照】旧字段提供默认值，显式包设置覆盖业务字段
/// @param config 主配置；paths 为应用目录；settings 为包设置；declared 为声明
/// @returns 内存中的配置与最小目录及 HTTP 能力，不修改持久配置
pub(super) fn resolve(
    config: &AppConfig,
    paths: &SaiPaths,
    settings: &Value,
    declared: &Capabilities,
) -> Result<RuntimeOverrides> {
    let mut merged = serde_json::to_value(&config.plugins.knowledge_base)?;
    let object = merged
        .as_object_mut()
        .context("knowledge base defaults must be an object")?;
    object.remove("enabled");
    object.insert(
        "language".into(),
        json!(if crate::i18n::is_zh() { "zh" } else { "en" }),
    );
    object.insert("input_paths".into(), json!([]));
    object.extend(
        settings
            .as_object()
            .context("knowledge-base settings must be an object")?
            .clone(),
    );
    let configured = object
        .get("data_dir")
        .and_then(Value::as_str)
        .context("knowledge-base.data_dir must be a string")?
        .trim();
    let root = if configured.is_empty() {
        paths.data_dir.join("kb").to_string_lossy().into_owned()
    } else {
        configured.to_string()
    };
    object.insert("data_dir".into(), json!(root));
    // 1. 【知识库兼容】【明确目录】数据库和正文只位于知识库根，导入来源不会取得写入权限
    let mut capabilities = declared.clone();
    capabilities.system.read_paths = [root.clone()].into();
    let inputs = object
        .get("input_paths")
        .and_then(Value::as_array)
        .context("knowledge-base.input_paths must be an array")?;
    anyhow::ensure!(
        inputs.len() <= 32,
        "knowledge-base.input_paths exceed 32 entries"
    );
    for input in inputs {
        capabilities.system.read_paths.insert(
            input
                .as_str()
                .context("knowledge-base input paths must be strings")?
                .to_string(),
        );
    }
    capabilities.binary.write_paths = [root.clone()].into();
    capabilities.system.remove_paths = [root].into();
    capabilities.http.clear();
    capabilities.http_read_only_post.clear();
    // 2. 【知识库兼容】【供应商投影】只交付选中的嵌入供应商，错误留到实际嵌入请求处理
    object.remove("provider");
    object.remove("provider_error");
    let id = object
        .get("embedding_provider_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let model = object
        .get("embedding_model")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if object.get("embedding_enabled").and_then(Value::as_bool) == Some(true)
        && !id.is_empty()
        && !model.is_empty()
    {
        match provider(config, paths, &id) {
            Ok(value) => {
                let endpoint = value["endpoint"]
                    .as_str()
                    .context("missing embedding endpoint")?;
                let url = reqwest::Url::parse(endpoint)?;
                capabilities.http.insert(url.origin().ascii_serialization());
                capabilities.http_read_only_post.insert(endpoint.into());
                object.insert("provider".into(), value);
            }
            Err(error) => {
                object.insert("provider_error".into(), json!(format!("{error:#}")));
            }
        }
    }
    capabilities.validate()?;
    Ok(RuntimeOverrides {
        settings: merged,
        capabilities,
    })
}

/// 【知识库兼容】【供应商凭据】复用供应商的单密钥、多密钥、环境变量和私密配置解析
/// @param config 主配置；paths 为私密配置路径；id 为唯一选中供应商
/// @returns 有效端点及凭据，错误不包含密钥值
fn provider(config: &AppConfig, paths: &SaiPaths, id: &str) -> Result<Value> {
    let provider = config.provider(Some(id))?;
    let endpoint = format!("{}/embeddings", provider.base_url.trim_end_matches('/'));
    let url = reqwest::Url::parse(&endpoint).context("invalid embedding endpoint")?;
    anyhow::ensure!(
        matches!(url.scheme(), "http" | "https")
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none(),
        "embedding endpoint must use HTTP(S) without credentials, query or fragment"
    );
    let api_key = provider
        .resolved_api_key(paths)
        .with_context(|| format!("embedding provider {id} has no api_key"))?;
    Ok(json!({"id":provider.id, "endpoint":url.as_str(), "api_key":api_key}))
}
