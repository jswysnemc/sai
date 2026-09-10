use serde::{Deserialize, Serialize};

/// 【插件视觉】【模型标识】宿主配置选出的视觉模型，不包含地址或凭据。
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisionModelInfo {
    pub provider_id: String,
    pub model: String,
}

/// 【插件视觉】【单图请求】图片由独立二进制句柄提供，插件只控制提示词与已识别的媒体类型。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VisionRequest {
    #[serde(default)]
    pub system: String,
    pub prompt: String,
    pub mime_type: String,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

/// 【插件视觉】【单次结果】返回视觉模型正文与可信模型标识，不执行任何工具建议。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VisionResponse {
    pub content: String,
    pub provider_id: String,
    pub model: String,
}

/// 【插件视觉】【输入边界】单图保持十 MiB 上限，媒体类型必须为支持的图片类型。
pub const MAX_VISION_IMAGE_BYTES: usize = 10 * 1024 * 1024;

impl VisionRequest {
    /// 【插件视觉】【请求校验】校验文字、媒体类型与实际图片字节数量，不信任模型提供的属性。
    /// @param image_bytes 实际缓冲字节数；max_text_bytes 为序列化文字限制
    /// @returns 参数有效时成功，不发起模型请求
    pub fn validate(&self, image_bytes: usize, max_text_bytes: usize) -> anyhow::Result<()> {
        if image_bytes == 0 || image_bytes > MAX_VISION_IMAGE_BYTES {
            anyhow::bail!("vision image must contain 1 to 10485760 bytes");
        }
        if self.prompt.trim().is_empty() {
            anyhow::bail!("vision prompt must not be empty");
        }
        if !matches!(
            self.mime_type.as_str(),
            "image/png" | "image/jpeg" | "image/jpg" | "image/gif" | "image/webp" | "image/bmp"
        ) {
            anyhow::bail!("unsupported vision image MIME type");
        }
        if serde_json::to_vec(self)?.len() > max_text_bytes {
            anyhow::bail!("plugin vision request exceeds size limit");
        }
        Ok(())
    }
}
