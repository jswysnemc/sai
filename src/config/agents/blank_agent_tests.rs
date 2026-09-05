use super::*;
use crate::config::AppConfig;

/// 构造一个自定义 Agent 档案并挂进配置。
///
/// 参数:
/// - `profile`: 待挂载的档案
///
/// 返回:
/// - 已包含该档案的配置
fn config_with(profile: AgentProfile) -> AppConfig {
    let mut config = AppConfig::default();
    config.agents = vec![profile];
    config
}

/// 构造一个最小可用的自定义档案。
///
/// 参数:
/// - `id`: 档案标识
///
/// 返回:
/// - 档案
fn profile(id: &str) -> AgentProfile {
    AgentProfile {
        id: id.to_string(),
        name: id.to_string(),
        ..AgentProfile::default()
    }
}

/// 验证关闭内置人设且未写提示词时得到空白提示词。
///
/// 这是"0 提示词 Agent"的核心：此前空提示词会被当成未设置，
/// 一路回退到内置人设，配置界面上怎么改都清不掉。
#[test]
fn a_blank_agent_produces_an_empty_base_prompt() {
    let dir = tempfile::tempdir().unwrap();
    let paths = crate::paths::SaiPaths::for_tests(dir.path());
    let mut blank = profile("blank");
    blank.prompt_sections = crate::config::PromptSectionToggles::all_disabled();

    let config =
        apply_agent_override(config_with(blank), Some("blank"), AgentSurface::Cli).unwrap();

    assert_eq!(config.base_system_prompt(&paths).unwrap(), "");
    assert_eq!(config.system_prompt(&paths).unwrap(), "");
}

/// 验证自己写了提示词时不受内置人设开关影响。
#[test]
fn an_explicit_prompt_survives_with_the_builtin_persona_off() {
    let dir = tempfile::tempdir().unwrap();
    let paths = crate::paths::SaiPaths::for_tests(dir.path());
    let mut custom = profile("custom");
    custom.system_prompt = "只回答是或否".to_string();
    custom.prompt_sections = crate::config::PromptSectionToggles::all_disabled();

    let config =
        apply_agent_override(config_with(custom), Some("custom"), AgentSurface::Cli).unwrap();

    assert_eq!(config.base_system_prompt(&paths).unwrap(), "只回答是或否");
}

/// 验证独占空白名单产生零工具覆盖。
///
/// 空列表在旧语义下表示"全量"，这里必须落成一个明确的空覆盖，
/// 否则 Agent 会拿到全部工具。
#[test]
fn an_exclusive_empty_whitelist_yields_no_tools() {
    let mut bare = profile("bare");
    bare.tools_exclusive = true;

    let config = apply_agent_override(config_with(bare), Some("bare"), AgentSurface::Cli).unwrap();

    let runtime = config.agent_runtime.expect("独占白名单必须落成覆盖");
    assert!(runtime.exclusive);
    assert!(runtime.enabled_tools.is_empty());
}

/// 验证独占白名单只保留列出的工具。
#[test]
fn an_exclusive_whitelist_keeps_only_what_it_lists() {
    let mut minimal = profile("minimal");
    minimal.tools_exclusive = true;
    minimal.enabled_tools = vec!["run_command".to_string(), "write_file".to_string()];

    let config =
        apply_agent_override(config_with(minimal), Some("minimal"), AgentSurface::Cli).unwrap();

    let runtime = config.agent_runtime.expect("独占白名单必须落成覆盖");
    assert_eq!(runtime.enabled_tools, vec!["run_command", "write_file"]);
}

/// 验证非独占的空白名单仍是旧语义。
///
/// 已有配置里存在空列表且依赖"继承全量"，语义翻转会让它们静默失去工具。
#[test]
fn a_non_exclusive_empty_whitelist_keeps_the_legacy_meaning() {
    let config = apply_agent_override(
        config_with(profile("legacy")),
        Some("legacy"),
        AgentSurface::Cli,
    )
    .unwrap();

    assert!(config.agent_runtime.is_none());
}
