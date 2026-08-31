use super::manager::StartRunRequest;
use anyhow::{bail, Result};

/// HTTP 请求体上限，只防异常大包打满内存，不是产品层的图片体积策略。
pub(crate) const MAX_RUN_REQUEST_BYTES: usize = 128 * 1024 * 1024;
const MAX_RUN_IMAGE_ATTACHMENTS: usize = 4;

/// 校验 Web 运行请求中的图片数量。
///
/// 单张编码体积不在 SAI 侧限制，由上游模型接口拒绝。
///
/// 参数:
/// - `request`: 待启动的 Web 运行请求
///
/// 返回:
/// - 请求在数量限制内时返回成功
pub(super) fn validate_start_request(request: &StartRunRequest) -> Result<()> {
    let image_urls = request
        .image_url
        .iter()
        .chain(request.image_urls.iter())
        .collect::<Vec<_>>();
    if image_urls.len() > MAX_RUN_IMAGE_ATTACHMENTS {
        bail!("a run accepts at most {MAX_RUN_IMAGE_ATTACHMENTS} image attachments");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::runs::manager::{QueueInsertAt, RunKind};

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
            insert_at: QueueInsertAt::Turn,
        }
    }

    #[test]
    fn rejects_too_many_image_attachments() {
        let request = request(vec!["data:image/png;base64,AA".to_string(); 5]);

        assert!(validate_start_request(&request).is_err());
    }

    /// 验证单张编码体积不再由 SAI 拦截。
    #[test]
    fn accepts_large_encoded_image_attachment() {
        let request = request(vec!["x".repeat(8 * 1024 * 1024)]);

        assert!(validate_start_request(&request).is_ok());
    }
}
