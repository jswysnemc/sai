use super::*;
use crate::tools::{self, ToolSpec};
use serde_json::{json, Value};

#[test]
fn progressive_visibility_starts_with_base_and_gateway_tools() {
    let mut registry = test_registry();
    tools::register_progressive_loader(&mut registry, &wildcard_deferred());
    let visibility = ToolVisibility::new(wildcard_deferred());
    let names = definition_names(visibility.definitions(&registry));

    assert_eq!(names, ["read_file", tools::LOAD_NAME, tools::INVOKE_NAME]);
    assert!(visibility.is_visible("read_file"));
    assert!(!visibility.is_visible("web_search"));
}

#[test]
fn anchored_standard_exposes_minimal_pair_then_resident_gateway() {
    let config = AppConfig::default();
    let mut registry = anchored_registry();
    let mut visibility = ToolVisibility::from_config_with_anchor(&config, true, false);
    let deferred = visibility.deferred_tools().to_vec();
    tools::register_progressive_loader(&mut registry, &deferred);

    assert_eq!(
        definition_names(visibility.definitions(&registry)),
        ["bash", "str_replace_editor"]
    );
    assert!(visibility.is_visible("bash"));
    assert!(!visibility.is_visible(tools::LOAD_NAME));
    assert!(visibility.is_visible("read_file"));

    assert!(visibility.promote_anchor());
    assert_eq!(
        definition_names(visibility.definitions(&registry)),
        [
            "bash",
            "str_replace_editor",
            tools::LOAD_NAME,
            tools::INVOKE_NAME,
        ]
    );
    assert!(visibility.is_visible(tools::LOAD_NAME));
    assert!(visibility.is_visible("write_file"));
}

#[test]
fn progressive_visibility_keeps_definitions_fixed_after_loading() {
    let mut registry = test_registry();
    tools::register_progressive_loader(&mut registry, &wildcard_deferred());
    let mut visibility = ToolVisibility::new(wildcard_deferred());

    load_args(
        &mut visibility,
        &registry,
        r#"{"type":"tool","keywords":["web_search"]}"#,
    );
    let names = definition_names(visibility.definitions(&registry));

    assert_eq!(names, ["read_file", tools::LOAD_NAME, tools::INVOKE_NAME]);
    assert!(visibility.is_visible("web_search"));
    assert!(!visibility.is_visible("analyze_image"));
}

/// 验证显式延迟配置只隐藏指定工具。
#[test]
fn explicit_deferred_list_keeps_other_tools_native() {
    let deferred = vec!["web_search".to_string()];
    let mut registry = test_registry();
    tools::register_progressive_loader(&mut registry, &deferred);
    let visibility = ToolVisibility::new(deferred);
    let names = definition_names(visibility.definitions(&registry));

    assert_eq!(
        names,
        [
            "read_file",
            "analyze_image",
            tools::LOAD_NAME,
            tools::INVOKE_NAME,
        ]
    );
    assert!(!visibility.is_visible("web_search"));
    assert!(visibility.is_visible("analyze_image"));
}

/// 验证加载任意工具后供应商工具数组保持逐字稳定。
#[test]
fn progressive_visibility_never_changes_provider_definitions() {
    let deferred = vec!["deferred_first".to_string(), "deferred_second".to_string()];
    let mut registry = ToolRegistry::new();
    for name in ["read_file", "deferred_first", "grep", "deferred_second"] {
        registry.register(ToolSpec::new(
            name,
            "test",
            json!({"type":"object","properties":{},"additionalProperties":false}),
            |_| async { Ok("ok".to_string()) },
        ));
    }
    tools::register_progressive_loader(&mut registry, &deferred);
    let mut visibility = ToolVisibility::new(deferred);

    let initial_definitions = visibility.definitions(&registry);
    assert_eq!(
        definition_names(initial_definitions.clone()),
        ["read_file", "grep", tools::LOAD_NAME, tools::INVOKE_NAME]
    );
    let initial = serde_json::to_value(initial_definitions).unwrap();

    load_args(
        &mut visibility,
        &registry,
        r#"{"type":"tool","keywords":["deferred_first"]}"#,
    );
    let after_first = serde_json::to_value(visibility.definitions(&registry)).unwrap();
    assert_eq!(after_first, initial);

    load_args(
        &mut visibility,
        &registry,
        r#"{"type":"tool","keywords":["deferred_second"]}"#,
    );
    let after_second = serde_json::to_value(visibility.definitions(&registry)).unwrap();
    assert_eq!(after_second, initial);
}

#[test]
fn progressive_visibility_reports_duplicate_tool_load() {
    let mut registry = test_registry();
    tools::register_progressive_loader(&mut registry, &wildcard_deferred());
    let mut visibility = ToolVisibility::new(wildcard_deferred());

    let first = load_args(
        &mut visibility,
        &registry,
        r#"{"type":"tool","keywords":["web_search"]}"#,
    );
    let second = load_args(
        &mut visibility,
        &registry,
        r#"{"type":"tool","keywords":["web_search"]}"#,
    );
    let first = serde_json::from_str::<Value>(&first).unwrap();
    let second = serde_json::from_str::<Value>(&second).unwrap();

    assert_eq!(first["already_loaded"], json!(false));
    assert_eq!(first["tools"][0]["name"], json!("web_search"));
    assert_eq!(
        first["tools"][0]["definition"]["function"]["name"],
        json!("web_search")
    );
    assert!(first["tools"][0]["definition"]["function"]["parameters"].is_object());
    assert_eq!(second["already_loaded"], json!(true));
    assert_eq!(second["tools"][0]["status"], json!("already_loaded"));
    assert!(second["instruction"]
        .as_str()
        .unwrap()
        .contains("Do not call load"));
}

#[test]
fn progressive_visibility_keeps_loader_description_stable_after_loading() {
    let mut registry = test_registry();
    tools::register_progressive_loader(&mut registry, &wildcard_deferred());
    let mut visibility = ToolVisibility::new(wildcard_deferred());

    let initial = visibility
        .definitions(&registry)
        .into_iter()
        .find(|definition| definition.function.name == tools::LOAD_NAME)
        .unwrap()
        .function
        .description;

    load_args(
        &mut visibility,
        &registry,
        r#"{"type":"tool","keywords":["web_search"]}"#,
    );
    let definitions = visibility.definitions(&registry);
    let description = definitions
        .iter()
        .find(|definition| definition.function.name == tools::LOAD_NAME)
        .unwrap()
        .function
        .description
        .as_str();

    assert_eq!(description, initial.as_str());
    assert!(!description.contains("Already loaded tools"));
    assert!(description.contains("Available groups"));
    assert!(description.contains("web_search"));
    assert!(description.contains("web"));
    assert!(description.contains("analyze_image"));
}

/// load 描述只反映当前 registry 中的可加载工具，因此会随 agent enabled_tools 过滤结果变化。
#[test]
fn loader_description_follows_agent_filtered_registry() {
    let mut registry = ToolRegistry::new();
    registry.register(ToolSpec::new(
        "read_file",
        "Read a file.",
        json!({"type":"object","properties":{},"additionalProperties":false}),
        |_| async { Ok("ok".to_string()) },
    ));
    registry.register(ToolSpec::new(
        "web_search",
        "Search the web.",
        json!({"type":"object","properties":{},"additionalProperties":false}),
        |_| async { Ok("ok".to_string()) },
    ));
    tools::register_progressive_loader(&mut registry, &wildcard_deferred());
    let visibility = ToolVisibility::new(wildcard_deferred());
    let description = visibility
        .definitions(&registry)
        .into_iter()
        .find(|definition| definition.function.name == tools::LOAD_NAME)
        .unwrap()
        .function
        .description;

    assert!(description.contains("web_search"));
    assert!(description.contains("Available groups"));
    assert!(!description.contains("read_file"));
    assert!(!description.contains("analyze_image"));
    assert!(!description.contains("deep_diagnose"));
}

#[test]
fn progressive_loader_loads_skill_document() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path());
    let skill_dir = paths.skills_dir.join("gpu-passthrough");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: gpu-passthrough\ndescription: GPU switching\n---\n\nUse `gpustoggle --status`.",
    )
    .unwrap();
    let registry = test_registry();
    let config = AppConfig::default();
    let mut visibility = ToolVisibility::new(wildcard_deferred());

    let output = visibility
        .load_from_arguments(
            &registry,
            r#"{"type":"skill","keywords":["gpu-passthrough"]}"#,
            &config,
            &paths,
        )
        .unwrap();

    let output = serde_json::from_str::<serde_json::Value>(&output).unwrap();
    assert!(output["skills"].is_array());
    assert!(output["skills"][0]["content"]
        .as_str()
        .unwrap()
        .contains("<loaded-skill"));
    assert!(output.to_string().contains("gpu-passthrough"));
    assert!(output.to_string().contains("gpustoggle --status"));
}

#[test]
fn progressive_loader_rejects_skill_when_disabled() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path());
    let registry = test_registry();
    let mut config = AppConfig::default();
    config.skills.enabled = false;
    let mut visibility = ToolVisibility::new(wildcard_deferred());

    let err = visibility
        .load_from_arguments(
            &registry,
            r#"{"type":"skill","keywords":["yce"]}"#,
            &config,
            &paths,
        )
        .unwrap_err();

    assert!(err.to_string().contains("skill loading is disabled"));
}

#[test]
fn progressive_visibility_restores_loaded_tools() {
    let mut registry = test_registry();
    tools::register_progressive_loader(&mut registry, &wildcard_deferred());
    let mut visibility = ToolVisibility::new(wildcard_deferred());

    visibility.restore_loaded_tools(
        &registry,
        &[
            "web_search".to_string(),
            "unknown_tool".to_string(),
            "read_file".to_string(),
        ],
    );
    let names = definition_names(visibility.definitions(&registry));

    assert_eq!(names, ["read_file", tools::LOAD_NAME, tools::INVOKE_NAME]);
    assert!(visibility.is_visible("web_search"));
    assert!(visibility.is_visible("read_file"));
    assert!(!visibility.is_visible("unknown_tool"));
    assert_eq!(
        visibility.loaded_tool_names(),
        vec!["web_search".to_string()]
    );
}

/// 构造「非基础工具一律需要 load」的延迟集合。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 只含通配符的延迟工具集合
fn wildcard_deferred() -> Vec<String> {
    vec![crate::config::DEFERRED_ALL_NON_BASE.to_string()]
}

fn test_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(ToolSpec::new(
        "read_file",
        "Read a file.",
        json!({"type":"object","properties":{},"additionalProperties":false}),
        |_| async { Ok("ok".to_string()) },
    ));
    registry.register(ToolSpec::new(
        "web_search",
        "Search the web.",
        json!({"type":"object","properties":{},"additionalProperties":false}),
        |_| async { Ok("ok".to_string()) },
    ));
    registry.register(ToolSpec::new(
        "analyze_image",
        "Analyze an image.",
        json!({"type":"object","properties":{},"additionalProperties":false}),
        |_| async { Ok("ok".to_string()) },
    ));
    registry
}

fn anchored_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    for name in ["run_command", "str_replace", "read_file", "write_file"] {
        registry.register(ToolSpec::new(
            name,
            "test",
            json!({"type":"object","properties":{},"additionalProperties":false}),
            |_| async { Ok("ok".to_string()) },
        ));
    }
    registry
}

fn load_args(visibility: &mut ToolVisibility, registry: &ToolRegistry, arguments: &str) -> String {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path());
    let config = AppConfig::default();
    visibility
        .load_from_arguments(registry, arguments, &config, &paths)
        .unwrap()
}

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

fn definition_names(definitions: Vec<ToolDefinition>) -> Vec<String> {
    definitions
        .into_iter()
        .map(|definition| definition.function.name)
        .collect()
}
