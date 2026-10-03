use super::*;
use crate::agent::ToolVisibility;
use crate::config::AgentRuntimeOverride;
use crate::paths::SaiPaths;

/// 【上下文】【工具配置回归】会话注册遵守白名单，重新绑定时移除已关闭工具
/// 参数: 无；返回无
#[test]
fn context_registration_respects_agent_whitelist_and_rebinding() {
    let dir = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(dir.path());
    let state = StateStore::new(&paths).unwrap();
    let mut config = AppConfig::default();
    config.context.experimental_context_blocks = true;
    let mut registry = ToolRegistry::new();
    register(&mut registry, &state, &config);
    assert!(NAMES.iter().all(|name| registry.contains(name)));
    config.agent_runtime = Some(AgentRuntimeOverride {
        enabled_tools: vec!["context_status".into(), "restore_context".into()],
        exclusive: true,
        ..Default::default()
    });
    register(&mut registry, &state, &config);
    assert!(registry.contains("context_status"));
    assert!(registry.contains("restore_context"));
    assert!(!registry.contains("compress_context"));
    assert!(!registry.contains("search_context"));
    config.context.experimental_context_blocks = false;
    register(&mut registry, &state, &config);
    assert!(NAMES.iter().all(|name| !registry.contains(name)));
}

/// 【上下文】【按需配置回归】显式延迟工具不直接暴露，默认通配符仍保留常驻上下文工具
/// 参数: 无；返回无
#[test]
fn context_visibility_respects_explicit_deferred_tools() {
    let mut config = AppConfig::default();
    config.context.experimental_context_blocks = true;
    config.agent_runtime = Some(AgentRuntimeOverride {
        deferred_tools: vec!["*".into(), "compress_context".into()],
        ..Default::default()
    });
    let mut registry = ToolRegistry::new();
    register_catalog(&mut registry);
    let visibility = ToolVisibility::from_config(&config);
    let names = visibility
        .definitions(&registry)
        .into_iter()
        .map(|item| item.function.name)
        .collect::<Vec<_>>();
    assert!(!names.iter().any(|name| name == "compress_context"));
    assert!(names.iter().any(|name| name == "context_status"));
    assert!(!visibility.is_visible("compress_context"));
    assert!(visibility.is_visible("context_status"));
}
