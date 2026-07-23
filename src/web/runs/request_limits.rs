use super::manager::StartRunRequest;
use anyhow::{bail, Result};

pub(crate) const MAX_RUN_REQUEST_BYTES: usize = 16 * 1024 * 1024;
const MAX_RUN_IMAGE_ATTACHMENTS: usize = 4;
const MAX_RUN_IMAGE_DATA_URL_BYTES: usize = 3 * 1024 * 1024;

/// 校验 Web 运行请求中的图片数量和单项编码尺寸。
///
/// 参数:
/// - `request`: 待启动的 Web 运行请求
///
/// 返回:
/// - 请求在限制内时返回成功
pub(super) fn validate_start_request(request: &StartRunRequest) -> Result<()> {
    let image_urls = request
        .image_url
        .iter()
        .chain(request.image_urls.iter())
        .collect::<Vec<_>>();
    if image_urls.len() > MAX_RUN_IMAGE_ATTACHMENTS {
        bail!("a run accepts at most {MAX_RUN_IMAGE_ATTACHMENTS} image attachments");
    }
    if image_urls
        .iter()
        .any(|url| url.len() > MAX_RUN_IMAGE_DATA_URL_BYTES)
    {
        bail!("an encoded image attachment exceeds {MAX_RUN_IMAGE_DATA_URL_BYTES} bytes");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::runs::manager::RunKind;

    /// 创建图片限制测试请求。
    fn request(image_urls: Vec<String>) -> StartRunRequest {
        StartRunRequest {
            kind: RunKind::Conversation,
            session_id: "session".to_string(),
            input: String::new(),
            agent_id: None,
            image_url: None,
            image_urls,
            mode: None,
            provider_id: None,
            model: None,
            thinking_level: None,
        }
    }

    #[test]
    fn rejects_too_many_image_attachments() {
        let request = request(vec!["data:image/png;base64,AA".to_string(); 5]);

        assert!(validate_start_request(&request).is_err());
    }

    #[test]
    fn rejects_oversized_encoded_image_attachment() {
        let request = request(vec!["x".repeat(MAX_RUN_IMAGE_DATA_URL_BYTES + 1)]);

        assert!(validate_start_request(&request).is_err());
    }
}
