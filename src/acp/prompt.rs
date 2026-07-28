use crate::agent_engine::TurnRequest;
use agent_client_protocol::schema::v1::{
    ContentBlock, EmbeddedResource, EmbeddedResourceResource, ImageContent, TextContent,
    TextResourceContents,
};
use anyhow::Result;

/// 把本轮输入转成 ACP 的内容块数组。
///
/// 参数:
/// - `request`: 本轮输入、图片与 Sai 动态上下文
/// - `capabilities`: agent 在握手中声明的提示能力
///
/// 返回:
/// - 可发送给 `session/prompt` 的内容块数组
pub(super) fn blocks(
    request: &TurnRequest,
    capabilities: &super::capabilities::AcpCapabilities,
) -> Result<Vec<ContentBlock>> {
    let mut blocks = vec![ContentBlock::Text(TextContent::new(&request.input))];
    // 【ACP】【提示组装】1. 只有声明 embeddedContext 的 agent 才接收记忆与目标资源
    if capabilities.embedded_context {
        for context in &request.contexts {
            let resource =
                TextResourceContents::new(&context.text, &context.uri).mime_type("text/markdown");
            blocks.push(ContentBlock::Resource(EmbeddedResource::new(
                EmbeddedResourceResource::TextResourceContents(resource),
            )));
        }
    }
    // 【ACP】【提示组装】2. 图片能力不支持时明确报错，避免静默丢失用户附件
    if !request.image_urls.is_empty() && !capabilities.prompt_image {
        anyhow::bail!("ACP agent does not advertise image prompt support");
    }
    for image in &request.image_urls {
        let Some((mime, data)) = split_data_url(image) else {
            continue;
        };
        blocks.push(ContentBlock::Image(ImageContent::new(data, mime)));
    }
    Ok(blocks)
}

/// 将 ACP turn usage 转换为 Sai 的统一用量结构。
///
/// 参数:
/// - `usage`: agent 在 prompt 响应中报告的用量
///
/// 返回:
/// - Sai 统一用量
pub(super) fn convert_usage(usage: agent_client_protocol::schema::v1::Usage) -> crate::llm::Usage {
    crate::llm::Usage {
        prompt_tokens: usage.input_tokens,
        completion_tokens: usage.output_tokens,
        total_tokens: usage.total_tokens,
        cache_read_tokens: usage.cached_read_tokens.unwrap_or_default(),
        cache_write_tokens: usage.cached_write_tokens.unwrap_or_default(),
    }
}

/// 拆分 data URL 的 MIME 类型与 base64 数据。
///
/// 参数:
/// - `url`: data URL
///
/// 返回:
/// - `(mime, base64)`；不是 base64 data URL 时返回 None
pub(super) fn split_data_url(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("data:")?;
    let (meta, data) = rest.split_once(',')?;
    let mime = meta.strip_suffix(";base64")?;
    Some((mime.to_string(), data.to_string()))
}
