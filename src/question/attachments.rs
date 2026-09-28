use super::QuestionRequest;
use anyhow::{bail, Result};
use base64::Engine;

/// 【结构化提问】【附件校验】确认图片归属、数量和编码有效，结构化外部表单只接收文字。
/// @param request 原始问题；image_urls 按题号分组的图片 data URL
/// @returns 校验结果，不修改待答状态
pub(super) fn validate(request: &QuestionRequest, image_urls: &[Vec<String>]) -> Result<()> {
    if image_urls.is_empty() {
        return Ok(());
    }
    if image_urls.len() != request.questions.len() {
        bail!("image group count does not match question count");
    }
    for (question, images) in request.questions.iter().zip(image_urls) {
        if images.is_empty() {
            continue;
        }
        if !question.custom || question.validation.is_some() {
            bail!("this question accepts text answers only");
        }
        if images.len() > 4 {
            bail!("attach at most four images per question");
        }
        for url in images {
            let Some((media, payload)) = url.split_once(";base64,") else {
                bail!("image must be a base64 data URL");
            };
            if !matches!(
                media,
                "data:image/png" | "data:image/jpeg" | "data:image/gif" | "data:image/webp"
            ) {
                bail!("unsupported image type");
            }
            if payload.is_empty()
                || base64::engine::general_purpose::STANDARD
                    .decode(payload)
                    .is_err()
            {
                bail!("image contains invalid base64 data");
            }
        }
    }
    Ok(())
}
