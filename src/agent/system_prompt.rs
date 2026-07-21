use super::instruction_files::load_instruction_prompt;
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::tools;
use anyhow::Result;

/// 组装基础系统提示，包括附加指令文件、技能目录和可选额外提示。
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
    // 1. Agent / persona / 用户身份
    let mut base_system_prompt = config.system_prompt(paths)?;

    // 2. 全局 AGENT.md 与项目 .AGENT.md / .CLAUDE.md 等附加指令（可按 Agent 关闭）
    if config.load_instruction_files {
        let instruction_prompt = load_instruction_prompt(paths);
        if !instruction_prompt.trim().is_empty() {
            base_system_prompt.push_str("\n\n");
            base_system_prompt.push_str(&instruction_prompt);
        }
    }

    // 3. Skills 目录（渐进加载时仅 catalog）
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

    // 4. 调用方注入的额外系统提示
    if let Some(prompt) = extra_system_prompt
        .map(str::trim)
        .filter(|prompt| !prompt.is_empty())
    {
        base_system_prompt.push_str("\n\n");
        base_system_prompt.push_str(prompt);
    }
    Ok(base_system_prompt)
}
