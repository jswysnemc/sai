use crate::config::AppConfig;
use crate::llm::OpenAiCompatibleClient;
use crate::paths::SaiPaths;
use anyhow::Result;

/// 压缩模型运行时依赖。
pub(super) struct CompactionRuntime {
    pub(super) client: OpenAiCompatibleClient,
    pub(super) label: String,
}

/// 解析压缩模型；未显式配置时沿用当前会话模型。
///
/// 参数:
/// - `config`: 已包含本轮会话模型覆盖的配置
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 压缩专用客户端与可读模型标签
pub(super) fn resolve_compaction_runtime(
    config: &AppConfig,
    paths: &SaiPaths,
) -> Result<CompactionRuntime> {
    let runtime_config = config.compaction_runtime_config()?;
    let choice = runtime_config.compaction_provider_model()?;
    Ok(CompactionRuntime {
        client: OpenAiCompatibleClient::from_config(&runtime_config, paths)?,
        label: choice.label(),
    })
}
