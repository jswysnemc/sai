//! 【工具方式】【默认设置】保存终端命令选择，后续启动沿用同一工具方式。

use crate::agent::AgentMode;
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use anyhow::Result;

/// 【工具方式】【保存默认】仅为有效的脚本模式命令保存全局默认值，不改权限设置。
/// 参数: paths 为配置路径，input 为已解析的模式命令，mode 为切换后的会话模式。
/// 返回: 命中脚本模式命令时为 true；读取或保存失败时返回错误。
pub(super) fn save_default_for_command(
    paths: &SaiPaths,
    input: &str,
    mode: AgentMode,
) -> Result<bool> {
    if !matches!(
        input.trim().to_ascii_lowercase().as_str(),
        "/code-model" | "/code-model on" | "/code-model off" | "/code-model only"
    ) {
        return Ok(false);
    }
    // 1. 【工具方式】【配置读取】读取磁盘最新配置，避免运行期覆盖其他入口已保存的设置
    let mut config = AppConfig::load_or_default(paths)?;
    // 2. 【工具方式】【配置保存】显式写入所选方式，不依赖缺省值；沿用私有权限和原子写入流程
    config.codemode.mode = mode.codemode();
    config.save(paths)?;
    Ok(true)
}

#[cfg(test)]
#[path = "codemode_preferences_tests.rs"]
mod tests;
