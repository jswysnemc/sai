use crate::config::ModelEndpointConfig;
use crate::tools::image_generation::request::{model_catalog_url, ImageAuth};
use serde_json::Value;

/// 【模型接入】【模型目录】请求兼容目录，复用生图协议地址与凭据。
/// 参数: endpoint 为完整草稿；返回: 去重模型列表或请求错误
pub(crate) async fn request_models(endpoint: &ModelEndpointConfig) -> anyhow::Result<Vec<String>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()?;
    let key = endpoint.resolved_api_key()?;
    let (url, auth) = model_catalog_url(endpoint);
    let mut last_error = String::from("model endpoint returned no result");
    for candidate in [url] {
        let mut request = client.get(&candidate).header("Accept", "application/json");
        if !key.is_empty() {
            request = match auth {
                ImageAuth::Bearer => request.bearer_auth(&key),
                ImageAuth::GeminiQueryKey => request.query(&[("key", key.as_str())]),
            };
        }
        let response = request.send().await?;
        let status = response.status();
        let body = response.text().await?;
        if status.is_success() {
            serde_json::from_str::<Value>(&body)?;
            return Ok(parse_models(&body));
        }
        last_error = format!("{status}: {body}");
        if status.as_u16() != 404 {
            break;
        }
    }
    anyhow::bail!(last_error)
}

/// 【模型接入】【目录解析】从 OpenAI 或 Gemini 响应提取模型标识。
/// 参数: body 为 JSON 正文；返回: 排序去重的模型列表
pub(crate) fn parse_models(body: &str) -> Vec<String> {
    let value = serde_json::from_str::<Value>(body).ok();
    let values = value
        .as_ref()
        .and_then(|value| value.get("data").or_else(|| value.get("models")))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut models = values
        .iter()
        .filter_map(|item| {
            item.as_str()
                .or_else(|| item.get("id").and_then(Value::as_str))
                .or_else(|| item.get("name").and_then(Value::as_str))
        })
        .map(str::trim)
        .map(|name| name.strip_prefix("models/").unwrap_or(name))
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    models.sort();
    models.dedup();
    models
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【模型接入】【目录回归】兼容 OpenAI 与 Gemini 目录，并保留可直接使用的模型标识。
    /// 参数: 无；返回: 无
    #[test]
    fn parses_both_image_catalog_formats() {
        assert_eq!(
            parse_models(r#"{"data":[{"id":"image-b"},{"id":"image-a"},{"id":"image-b"}]}"#),
            ["image-a", "image-b"]
        );
        assert_eq!(
            parse_models(r#"{"models":[{"name":"models/gemini-image"}]}"#),
            ["gemini-image"]
        );
    }
}
