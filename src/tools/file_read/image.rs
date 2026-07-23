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
/// 参数:
/// - `config`: 已应用会话模型覆盖的应用配置
///
/// 返回:
/// - 当前图片应直接附加或交给备用视觉模型描述
pub(super) fn image_read_mode(config: &AppConfig) -> ImageReadMode {
    if !config.plugins.vision.prefer_current_multimodal_model {
        return ImageReadMode::DescribeWithConfiguredVisionModel;
    }
    let supports_vision = config.provider(None).ok().is_some_and(|provider| {
        provider
            .model_tags_for(&provider.default_model)
            .iter()
            .any(|tag| tag == MODEL_TAG_VISION)
    });
    if supports_vision {
        ImageReadMode::AttachToCurrentModel
    } else {
        ImageReadMode::DescribeWithConfiguredVisionModel
    }
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
    if !config.plugins.vision.enabled {
        bail!("vision plugin is disabled")
    }
    let prompt = request
        .image_prompt
        .as_deref()
        .unwrap_or(DEFAULT_IMAGE_PROMPT);
    match image_read_mode(config) {
        ImageReadMode::AttachToCurrentModel if !request.accept_model_attachment => {
            let description = crate::tools::vision::analyze_local_image_with_prompt(
                config,
                paths,
                &request.path,
                prompt,
            )
            .await?;
            Ok(ReadPage::text(json!({
                "type": "image-analysis",
                "path": request.path.display().to_string(),
                "prompt": prompt,
                "attachment_submitted": false,
                "content": description,
            })))
        }
        ImageReadMode::AttachToCurrentModel => {
            let image_url = crate::tools::vision::local_image_data_url(&request.path)?;
            let attachment =
                ToolModelAttachment::new(image_url, request.path.display().to_string(), prompt);
            Ok(ReadPage {
                value: json!({
                    "type": "image-attachment",
                    "path": request.path.display().to_string(),
                    "prompt": prompt,
                    "attachment_submitted": true,
                    "content": "The image is attached to the active multimodal model for direct analysis.",
                }),
                model_attachments: vec![attachment],
            })
        }
        ImageReadMode::DescribeWithConfiguredVisionModel => {
            let description = crate::tools::vision::analyze_local_image_with_prompt(
                config,
                paths,
                &request.path,
                prompt,
            )
            .await?;
            Ok(ReadPage::text(json!({
                "type": "image-analysis",
                "path": request.path.display().to_string(),
                "prompt": prompt,
                "attachment_submitted": false,
                "content": description,
            })))
        }
    }
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

    /// 当前会话模型不支持视觉时使用配置的备用视觉模型。
    #[test]
    fn text_only_model_uses_configured_vision_description() {
        let config = AppConfig::default();

        assert_eq!(
            image_read_mode(&config),
            ImageReadMode::DescribeWithConfiguredVisionModel
        );
    }

    /// 显式关闭当前模型优先策略时仍使用配置的备用视觉模型。
    #[test]
    fn disabled_preference_uses_configured_vision_description() {
        let mut config = AppConfig::default();
        mark_current_model_as_vision(&mut config);
        config.plugins.vision.prefer_current_multimodal_model = false;

        assert_eq!(
            image_read_mode(&config),
            ImageReadMode::DescribeWithConfiguredVisionModel
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
