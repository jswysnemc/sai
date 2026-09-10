use super::{stream_budget::StreamBudget, PluginServices, CALL_CHAIN};
use crate::config::{AppConfig, ProviderConfig};
use crate::llm::{ChatMessage, OpenAiCompatibleClient};
use crate::paths::SaiPaths;
use anyhow::{bail, Context, Result};
use base64::Engine;
use sai_plugin_runtime::host::{BinaryData, VisionModelInfo, VisionRequest, VisionResponse};
use serde_json::json;
use std::sync::{atomic::Ordering, Arc};

/// 【插件视觉】【可信来源】保存宿主配置快照，Lua 无法覆盖供应商、模型、地址或密钥。
#[derive(Clone)]
pub(crate) struct PluginVisionSource {
    configuration: Arc<(AppConfig, SaiPaths)>,
}

impl PluginVisionSource {
    /// 【插件视觉】【绑定配置】将已加载主配置绑定到工具注册表，初始化不发送请求。
    /// @param configuration 主配置及应用路径的共享快照
    /// @returns 独立于 Agent 文本模型的视觉来源
    pub(crate) fn new(configuration: Arc<(AppConfig, SaiPaths)>) -> Self {
        Self { configuration }
    }

    /// 【插件视觉】【配置选择】沿用视觉供应商覆盖及视觉模型覆盖规则。
    /// @returns 已启用时的完整宿主配置；禁用时返回 None，错误不包含凭据
    fn provider(&self) -> Result<Option<ProviderConfig>> {
        let config = &self.configuration.0;
        let vision = &config.plugins.vision;
        if !vision.enabled {
            return Ok(None);
        }
        let id = vision.vision_provider_id.trim();
        let mut provider = config.provider((!id.is_empty()).then_some(id))?.clone();
        let model = vision.vision_model.trim();
        if !model.is_empty() {
            provider.default_model = model.to_string();
        }
        if provider.default_model.trim().is_empty() {
            bail!("vision provider has no active model");
        }
        if !provider.models.contains(&provider.default_model) {
            provider.models.push(provider.default_model.clone());
        }
        Ok(Some(provider))
    }

    /// 【插件视觉】【模型信息】只返回配置选中的公开标识，不初始化客户端或读取密钥文件。
    /// @returns 可用配置的供应商与模型名称；禁用时返回 None
    pub(super) fn info(&self) -> Result<Option<VisionModelInfo>> {
        Ok(self.provider()?.map(|provider| VisionModelInfo {
            provider_id: provider.id,
            model: provider.default_model,
        }))
    }
}

impl PluginServices {
    /// 【插件视觉】【分项绑定】仅在插件取得视觉授权后提供视觉服务。
    /// @param source 授权后的视觉来源，None 表示无能力
    /// @returns 绑定该来源的本次调用服务
    pub(crate) fn with_vision(mut self, source: Option<PluginVisionSource>) -> Self {
        self.vision = source;
        self
    }

    /// 【插件视觉】【模型执行】将有界图片编码为一次无工具的视觉请求，筛选规则仍由 Lua 决定。
    /// @param request 提示词及媒体类型；image 为预算租约；max_bytes 为文字输入输出上限
    /// @returns 模型正文及可信模型标识
    pub(super) async fn complete_vision(
        &self,
        request: VisionRequest,
        image: BinaryData,
        max_bytes: usize,
    ) -> Result<VisionResponse> {
        request.validate(image.bytes().len(), max_bytes)?;
        let source = self
            .vision
            .as_ref()
            .context("plugin vision service is unavailable")?;
        let provider = source.provider()?.context("vision service is disabled")?;
        let client = OpenAiCompatibleClient::new(
            &provider,
            &source.configuration.0,
            &source.configuration.1,
        )?;
        let encoded = base64::engine::general_purpose::STANDARD.encode(image.bytes());
        let image_url = format!("data:{};base64,{encoded}", request.mime_type);
        let mut messages = Vec::new();
        if !request.system.is_empty() {
            messages.push(ChatMessage::system(request.system));
        }
        messages.push(ChatMessage::user_with_image(request.prompt, image_url));
        let mut budget = StreamBudget::new(max_bytes);
        let operation =
            client.chat_stream_events(messages, Vec::new(), |event| budget.observe(&event));
        let round = self.rounds.fetch_add(1, Ordering::AcqRel);
        let events = self.events();
        // 1. 【插件视觉】【事件归属】使用隔离实例和当前操作，不把模型输出作为宿主工具自动执行
        let result = CALL_CHAIN
            .scope(
                self.chain.clone(),
                crate::runtime_cwd::scope(self.workdir.clone(), async {
                    events
                        .model_round(json!({"kind":"plugin_vision", "round":round}), operation)
                        .await
                }),
            )
            .await?;
        // 2. 【插件视觉】【缓冲归还】整个请求结束后再释放图片租约，取消同样由 Future 析构释放
        drop(image);
        let response = VisionResponse {
            content: result.content,
            provider_id: provider.id,
            model: provider.default_model,
        };
        if serde_json::to_vec(&response)?.len() > max_bytes {
            bail!("plugin vision response exceeds size limit");
        }
        Ok(response)
    }
}
