use super::ToolVisibility;
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::tools::{ToolRegistry, ToolSpec};
use serde_json::{json, Value};

#[test]
fn loads_one_tool_with_public_argument() {
    let registry = test_registry();
    let mut visibility = ToolVisibility::new(true);

    let output = load(
        &mut visibility,
        &registry,
        r#"{"type":"tool","keywords":["web_search"]}"#,
    );
    let output = serde_json::from_str::<Value>(&output).unwrap();

    assert_eq!(output["tools"][0]["name"], json!("web_search"));
    assert!(visibility.is_visible("web_search"));
    assert!(!visibility.is_visible("web_fetch"));
}

#[test]
fn loads_multiple_tools_and_deduplicates_names() {
    let registry = test_registry();
    let mut visibility = ToolVisibility::new(true);

    let output = load(
        &mut visibility,
        &registry,
        r#"{"type":"tool","keywords":["web_search","analyze_image","web_search"]}"#,
    );
    let output = serde_json::from_str::<Value>(&output).unwrap();

    assert_eq!(output["tools"].as_array().unwrap().len(), 2);
    assert!(visibility.is_visible("web_search"));
    assert!(visibility.is_visible("analyze_image"));
}

#[test]
fn classifies_mixed_multiple_tool_load() {
    let registry = test_registry();
    let mut visibility = ToolVisibility::new(true);
    load(
        &mut visibility,
        &registry,
        r#"{"type":"tool","keywords":["web_search"]}"#,
    );

    let output = load(
        &mut visibility,
        &registry,
        r#"{"type":"tool","keywords":["web_search","analyze_image"]}"#,
    );
    let output = serde_json::from_str::<Value>(&output).unwrap();

    assert_eq!(output["tools"][0]["status"], json!("already_loaded"));
    assert_eq!(output["tools"][1]["status"], json!("loaded"));
    assert_eq!(output["already_loaded"], json!(false));
}

#[test]
fn rejects_invalid_keyword_arrays() {
    let registry = test_registry();
    for arguments in [
        r#"{"type":"tool","keywords":[]}"#,
        r#"{"type":"tool","keywords":["web_search",2]}"#,
        r#"{"type":"tool","keywords":["web_search",""]}"#,
    ] {
        let mut visibility = ToolVisibility::new(true);
        let error = load_error(&mut visibility, &registry, arguments);

        assert!(error.contains("keywords"));
        assert!(visibility.loaded_tool_names().is_empty());
    }
}

#[test]
fn rejects_conflicting_keyword_sources() {
    let registry = test_registry();
    let mut visibility = ToolVisibility::new(true);

    let error = load_error(
        &mut visibility,
        &registry,
        r#"{"type":"tool","keywords":["web_search"],"tools":["web_fetch"]}"#,
    );

    assert!(error.contains("one keywords source"));
    assert!(visibility.loaded_tool_names().is_empty());
}

#[test]
fn rejects_unknown_batch_atomically() {
    let registry = test_registry();
    let mut visibility = ToolVisibility::new(true);

    let error = load_error(
        &mut visibility,
        &registry,
        r#"{"type":"tool","keywords":["web_search","missing_tool"]}"#,
    );

    assert!(error.contains("unknown tool: missing_tool"));
    assert!(visibility.loaded_tool_names().is_empty());
}

/// 创建覆盖基础、同组和跨组场景的工具注册表。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 测试用工具注册表
fn test_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    for name in ["read_file", "web_search", "web_fetch", "analyze_image"] {
        registry.register(ToolSpec::new(
            name,
            format!("Test tool {name}."),
            json!({"type":"object","properties":{},"additionalProperties":false}),
            |_| async { Ok("ok".to_string()) },
        ));
    }
    registry
}

/// 执行一次成功的加载请求。
///
/// 参数:
/// - `visibility`: 工具可见性状态
/// - `registry`: 测试工具注册表
/// - `arguments`: 加载参数 JSON
///
/// 返回:
/// - 加载结果 JSON
fn load(visibility: &mut ToolVisibility, registry: &ToolRegistry, arguments: &str) -> String {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path());
    visibility
        .load_from_arguments(registry, arguments, &AppConfig::default(), &paths)
        .unwrap()
}

/// 执行一次预期失败的加载请求。
///
/// 参数:
/// - `visibility`: 工具可见性状态
/// - `registry`: 测试工具注册表
/// - `arguments`: 加载参数 JSON
///
/// 返回:
/// - 错误文本
fn load_error(visibility: &mut ToolVisibility, registry: &ToolRegistry, arguments: &str) -> String {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path());
    visibility
        .load_from_arguments(registry, arguments, &AppConfig::default(), &paths)
        .unwrap_err()
        .to_string()
}

/// 创建隔离的应用路径集合。
///
/// 参数:
/// - `root`: 临时目录根路径
///
/// 返回:
/// - 测试用应用路径
fn test_paths(root: &std::path::Path) -> SaiPaths {
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
