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
pub(crate) fn build_base_system_prompt(
    config: &AppConfig,
    paths: &SaiPaths,
    tools_enabled: bool,
    extra_system_prompt: Option<&str>,
) -> Result<String> {
    // 1. Agent 人设与用户身份
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
    if tools_enabled && config.skills.enabled && config.prompt_sections.skills_catalog {
        let progressive = !config.agent_deferred_tools().is_empty();
        let prompt = if progressive {
            tools::skills_catalog_prompt(config, paths)?
        } else {
            tools::skills_prompt(config, paths)?
        };
        if !prompt.trim().is_empty() {
            base_system_prompt.push_str("\n\n");
            base_system_prompt.push_str(&prompt);
        }
    }

    // 4. 状态覆盖契约，具体状态通过后续 user 标签增量更新
    if config.prompt_sections.state_contract {
        base_system_prompt.push_str("\n\n");
        base_system_prompt.push_str(super::runtime_context::CONTEXT_STATE_CONTRACT);
    }

    // 5. 记忆使用契约；注入的索引只说明有哪些记忆，不说明何时该写新的
    if tools_enabled && config.memory_config().enabled && config.prompt_sections.memory_contract {
        base_system_prompt.push_str("\n\n");
        base_system_prompt.push_str(crate::memory::file_store::memory_contract());
    }

    // 6. 调用方注入的额外系统提示
    if let Some(prompt) = extra_system_prompt
        .map(str::trim)
        .filter(|prompt| !prompt.is_empty())
    {
        base_system_prompt.push_str("\n\n");
        base_system_prompt.push_str(prompt);
    }
    Ok(base_system_prompt.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一份指向临时目录的配置与路径。
    ///
    /// 参数:
    /// - `root`: 临时根目录
    ///
    /// 返回:
    /// - （配置，路径集合）
    fn setup(root: &std::path::Path) -> (AppConfig, SaiPaths) {
        (AppConfig::default(), SaiPaths::for_tests(root))
    }

    /// 验证记忆启用时系统提示词带上使用契约。
    ///
    /// 契约与工具注册是两处独立的开关，只接对一处就会出现「工具在但模型
    /// 不知道何时该用」或反过来。这条测试锁的是接线本身。
    #[test]
    fn the_memory_contract_is_injected_when_memory_is_on() {
        let dir = tempfile::tempdir().unwrap();
        let (config, paths) = setup(dir.path());

        let prompt = build_base_system_prompt(&config, &paths, true, None).unwrap();

        assert!(prompt.contains("write_memory") || prompt.contains("read_memory"));
    }

    /// 验证记忆关闭时不注入契约。
    ///
    /// 工具没注册却讲一堆用法，只会诱导模型调用不存在的工具。
    #[test]
    fn no_memory_contract_when_memory_is_off() {
        let dir = tempfile::tempdir().unwrap();
        let (mut config, paths) = setup(dir.path());
        config.plugins.memory.enabled = false;
        config.memory.enabled = false;

        let prompt = build_base_system_prompt(&config, &paths, true, None).unwrap();

        assert!(!prompt.contains("read_memory"));
    }

    /// 验证禁用工具时同样不注入契约。
    #[test]
    fn no_memory_contract_without_tools() {
        let dir = tempfile::tempdir().unwrap();
        let (config, paths) = setup(dir.path());

        let prompt = build_base_system_prompt(&config, &paths, false, None).unwrap();

        assert!(!prompt.contains("read_memory"));
    }
}
