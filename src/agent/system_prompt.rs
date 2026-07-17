use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::tools;
use anyhow::Result;

/// 组装基础系统提示，包括技能目录和可选额外提示。
///
/// 参数:
/// - `config`: 应用配置
/// - `paths`: Sai 路径
/// - `tools_enabled`: 是否启用工具
/// - `extra_system_prompt`: 可选额外系统提示
///
/// 返回:
/// - 基础系统提示文本
pub(super) fn build_base_system_prompt(
    config: &AppConfig,
    paths: &SaiPaths,
    tools_enabled: bool,
    extra_system_prompt: Option<&str>,
) -> Result<String> {
    let mut base_system_prompt = config.system_prompt(paths)?;
    if tools_enabled && config.skills.enabled {
        let prompt = if config.tools.progressive_loading_enabled {
            tools::skills_catalog_prompt(config, paths)?
        } else {
            tools::skills_prompt(config, paths)?
        };
        if !prompt.trim().is_empty() {
            base_system_prompt.push_str("\n\n");
            base_system_prompt.push_str(&prompt);
        }
    }
    if let Some(prompt) = extra_system_prompt
        .map(str::trim)
        .filter(|prompt| !prompt.is_empty())
    {
        base_system_prompt.push_str("\n\n");
        base_system_prompt.push_str(prompt);
    }
    Ok(base_system_prompt)
}
