use super::{ReadPage, ReadRequest};
use crate::config::{AppConfig, MODEL_TAG_VISION};
use crate::paths::SaiPaths;
use crate::tools::ToolModelAttachment;
use anyhow::{bail, Result};
use serde_json::json;

const DEFAULT_IMAGE_PROMPT: &str = "请简洁描述这张图片，并指出重要细节。";

/// 图片读取时采用的模型处理方式。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ImageReadMode {
    AttachToCurrentModel,
    DescribeWithConfiguredVisionModel,
}

/// 根据当前会话模型能力选择图片读取方式。
///
/// 当前模型带视觉标签，或配置要求优先使用当前多模态模型时，
/// 把图片直接交给这次对话的模型，不再另开视觉描述请求。
///
/// 参数:
/// - `config`: 已应用会话模型覆盖的应用配置
///
/// 返回:
/// - 当前图片应直接附加或交给备用视觉模型描述
pub(super) fn image_read_mode(config: &AppConfig) -> ImageReadMode {
    if current_model_supports_vision(config)
        || config.plugins.vision.prefer_current_multimodal_model
    {
        return ImageReadMode::AttachToCurrentModel;
    }
    ImageReadMode::DescribeWithConfiguredVisionModel
}

/// 判断当前会话模型是否带视觉能力标签。
///
/// 参数:
/// - `config`: 已应用会话模型覆盖的应用配置
///
/// 返回:
/// - 当前模型带 vision 标签时为真
fn current_model_supports_vision(config: &AppConfig) -> bool {
    config.provider(None).ok().is_some_and(|provider| {
        provider
            .model_tags_for(&provider.default_model)
            .iter()
            .any(|tag| tag == MODEL_TAG_VISION)
    })
}

/// 读取本地图片，并按当前模型能力直接附加或生成备用描述。
///
/// 参数:
/// - `request`: 图片读取请求
/// - `config`: 已应用当前会话模型覆盖的配置
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 图片读取结果和可选的下一次模型请求附件
pub(super) async fn read_image_page(
    request: &ReadRequest,
    config: &AppConfig,
    paths: &SaiPaths,
) -> Result<ReadPage> {
    let prompt = request
        .image_prompt
        .as_deref()
        .unwrap_or(DEFAULT_IMAGE_PROMPT);
    match image_read_mode(config) {
        ImageReadMode::AttachToCurrentModel if request.accept_model_attachment => {
            let image_url = crate::tools::vision::local_image_data_url(&request.path)?;
            let attachment =
                ToolModelAttachment::new(image_url, request.path.display().to_string(), prompt);
            Ok(ReadPage {
                value: json!({
                    "type": "image-attachment",
                    "path": request.path.display().to_string(),
                    "prompt": prompt,
                    "attachment_submitted": true,
                    "content": "The image is attached to the current model for direct analysis.",
                }),
                model_attachments: vec![attachment],
            })
        }
        ImageReadMode::AttachToCurrentModel => {
            // 调用方不会转发附件时，只能退回文字描述
            describe_image_page(request, config, paths, prompt).await
        }
        ImageReadMode::DescribeWithConfiguredVisionModel => {
            describe_image_page(request, config, paths, prompt).await
        }
    }
}

/// 使用配置的视觉模型生成图片文字描述。
///
/// 参数:
/// - `request`: 图片读取请求
/// - `config`: 应用配置
/// - `paths`: Sai 路径集合
/// - `prompt`: 读图提示
///
/// 返回:
/// - 仅含文字描述的读取结果
async fn describe_image_page(
    request: &ReadRequest,
    config: &AppConfig,
    paths: &SaiPaths,
    prompt: &str,
) -> Result<ReadPage> {
    if !config.plugins.vision.enabled {
        bail!("vision plugin is disabled")
    }
    let description =
        crate::tools::vision::analyze_local_image_with_prompt(config, paths, &request.path, prompt)
            .await?;
    Ok(ReadPage::text(json!({
        "type": "image-analysis",
        "path": request.path.display().to_string(),
        "prompt": prompt,
        "attachment_submitted": false,
        "content": description,
    })))
}

#[cfg(test)]
mod tests {
    use super::{image_read_mode, read_image_page, ImageReadMode};
    use crate::config::{AppConfig, MODEL_TAG_VISION};
    use crate::paths::SaiPaths;
    use crate::tools::file_read::ReadRequest;
    use std::path::{Path, PathBuf};

    /// 为当前默认模型添加视觉能力标签。
    ///
    /// 参数:
    /// - `config`: 待修改配置
    ///
    /// 返回:
    /// - 无
    fn mark_current_model_as_vision(config: &mut AppConfig) {
        let active_provider = config.active_provider.clone();
        let provider = config
            .providers
            .iter_mut()
            .find(|provider| provider.id == active_provider)
            .unwrap();
        let model = provider.default_model.clone();
        provider
            .model_metadata
            .entry(model)
            .or_default()
            .tags
            .push(MODEL_TAG_VISION.to_string());
    }

    /// 构造隔离测试路径。
    ///
    /// 参数:
    /// - `root`: 临时目录
    ///
    /// 返回:
    /// - 测试用 Sai 路径集合
    fn test_paths(root: &Path) -> SaiPaths {
        SaiPaths {
            config_dir: root.join("config"),
            config_file: root.join("config/config.jsonc"),
            secrets_file: root.join("config/secrets.jsonc"),
            skills_dir: root.join("config/skills"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            state_dir: root.join("state"),
            pictures_dir: root.join("pictures"),
            fish_hook_file: root.join("fish/sai.fish"),
            bash_hook_file: root.join("shell/bash-hook.sh"),
            zsh_hook_file: root.join("shell/zsh-hook.zsh"),
            powershell_hook_file: root.join("shell/powershell-hook.ps1"),
        }
    }

    /// 当前会话模型支持视觉时直接提交图片附件。
    #[test]
    fn current_vision_model_uses_direct_attachment() {
        let mut config = AppConfig::default();
        mark_current_model_as_vision(&mut config);

        assert_eq!(
            image_read_mode(&config),
            ImageReadMode::AttachToCurrentModel
        );
    }

    /// 默认优先当前模型时，即使未打视觉标签也把图片交给当前会话模型。
    #[test]
    fn default_preference_attaches_to_current_model() {
        let config = AppConfig::default();

        assert_eq!(
            image_read_mode(&config),
            ImageReadMode::AttachToCurrentModel
        );
    }

    /// 关闭当前模型优先且模型无视觉标签时，才走备用视觉描述。
    #[test]
    fn disabled_preference_without_vision_uses_description() {
        let mut config = AppConfig::default();
        config.plugins.vision.prefer_current_multimodal_model = false;

        assert_eq!(
            image_read_mode(&config),
            ImageReadMode::DescribeWithConfiguredVisionModel
        );
    }

    /// 当前模型带视觉标签时，即使关闭优先策略仍直接附加。
    #[test]
    fn vision_tag_attaches_even_when_preference_is_disabled() {
        let mut config = AppConfig::default();
        mark_current_model_as_vision(&mut config);
        config.plugins.vision.prefer_current_multimodal_model = false;

        assert_eq!(
            image_read_mode(&config),
            ImageReadMode::AttachToCurrentModel
        );
    }

    /// 直接模式只把 data URL 放入模型附件，不写入工具协议 JSON。
    #[tokio::test]
    async fn direct_image_read_returns_single_ephemeral_attachment() {
        let temp = tempfile::tempdir().unwrap();
        let image = temp.path().join("sample.png");
        std::fs::write(&image, [0x89, b'P', b'N', b'G']).unwrap();
        let mut config = AppConfig::default();
        mark_current_model_as_vision(&mut config);
        let request = ReadRequest {
            path: PathBuf::from(&image),
            offset: 1,
            limit: 1,
            image_prompt: Some("读取图片文字".to_string()),
            accept_model_attachment: true,
        };

        let page = read_image_page(&request, &config, &test_paths(temp.path()))
            .await
            .unwrap();

        assert_eq!(page.model_attachments.len(), 1);
        assert!(page.model_attachments[0]
            .image_url
            .starts_with("data:image/png;base64,"));
        assert_eq!(page.value["attachment_submitted"], true);
        assert!(!page.value.to_string().contains("base64,"));
    }
}
