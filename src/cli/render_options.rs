use crate::config::{AppConfig, ProviderConfig};
use crate::render::StreamRenderOptions;

/// 组装流式渲染附加选项。
///
/// 参数:
/// - `config`: 当前应用配置
///
/// 返回:
/// - 流式渲染选项，包含等待动效详情
pub(super) fn stream_render_options(config: &AppConfig) -> StreamRenderOptions {
    let provider = config.provider(None).ok();
    StreamRenderOptions {
        readable_tool_names: config.display.readable_tool_names,
        wait_model: config
            .display
            .wait_show_model
            .then(|| provider.and_then(active_model_label))
            .flatten(),
        wait_thinking_level: config
            .display
            .wait_show_thinking_level
            .then(|| provider.map(active_thinking_level_label))
            .flatten(),
    }
}

/// 生成当前模型展示文本。
///
/// 参数:
/// - `provider`: 当前 Provider 配置
///
/// 返回:
/// - 展示用 Provider/模型文本，模型为空时返回空
fn active_model_label(provider: &ProviderConfig) -> Option<String> {
    let model = provider.default_model.trim();
    if model.is_empty() {
        return None;
    }
    let provider_name = provider.display_name.trim();
    if provider_name.is_empty() {
        Some(model.to_string())
    } else {
        Some(format!("{provider_name}/{model}"))
    }
}

/// 生成当前思考等级展示文本。
///
/// 参数:
/// - `provider`: 当前 Provider 配置
///
/// 返回:
/// - 展示用思考等级文本，空值按 auto 展示
fn active_thinking_level_label(provider: &ProviderConfig) -> String {
    let level = provider.thinking_level.trim();
    if level.is_empty() {
        "auto".to_string()
    } else {
        level.to_string()
    }
}
