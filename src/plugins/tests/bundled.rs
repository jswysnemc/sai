use super::support::{runtime, FixtureHost};
use sai_plugin_runtime::InvocationContext;
use serde_json::{json, Value};
use std::sync::Arc;

/// 【插件测试】【手册搜索】真实 Lua 解析链接，处理重复结果、单引号和非 ASCII 查询。
#[tokio::test]
async fn online_man_search_returns_clean_deduplicated_links() {
    let host = Arc::new(FixtureHost::new(&[(
        200,
        r#"
        <a href="/man/core/man-pages/printf.3.en">printf</a>
        <a href='/man/core/man-pages/printf.3p.en'>printf duplicate</a>
        <a href="/man/core/man-pages/printf.h.3head.en">printf.h</a>
        <a href="/man/sprintf.3.en">sprintf</a>
        <a href="https://unrelated.test/man/not-a-result">ignore</a>
    "#,
    )]));
    let plugin = runtime("online-man", host.clone());
    let result = plugin
        .call_tool(
            "online_man_search",
            json!({"query":"打印 format","section":"3"}),
            InvocationContext::default(),
        )
        .await
        .unwrap();
    assert_eq!(result, "- printf: https://man.archlinux.org/man/core/man-pages/printf.3.en\n- printf.h: https://man.archlinux.org/man/core/man-pages/printf.h.3head.en\n- sprintf: https://man.archlinux.org/man/sprintf.3.en");
    assert_eq!(
        host.requests.lock().unwrap()[0].url,
        "https://man.archlinux.org/search?q=%E6%89%93%E5%8D%B0%20format&lang=en&section=3"
    );
}

/// 【插件测试】【手册来源】指定章节的 Arch 请求失败后读取 man7，保留来源和中文正文。
#[tokio::test]
async fn online_man_falls_back_to_man7_without_losing_text() {
    let host = Arc::new(FixtureHost::new(&[
        (404, "missing"),
        (200, "<h1>printf</h1><p>格式化输出</p>"),
    ]));
    let plugin = runtime("online-man", host.clone());
    let result = plugin
        .call_tool(
            "online_man_get_page",
            json!({"name":"printf","section":"3"}),
            InvocationContext::default(),
        )
        .await
        .unwrap();
    assert!(result.starts_with("Source: https://man7.org/linux/man-pages/man3/printf.3.html\n\n"));
    assert!(result.contains("格式化输出"));
    let requests = host.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[0].url,
        "https://man.archlinux.org/man/printf.3.en.txt"
    );
}

/// 【插件测试】【服务状态契约】覆盖组件、严重程度、时间线、中文及 Next.js 字符串转义。
#[tokio::test]
async fn deepseek_lua_preserves_the_existing_response_contract() {
    let incident = json!({
        "change_id":"incident-1", "title":"API \"问题\"", "type":"incident", "status":"partial_outage",
        "start_at_seconds":0.9, "close_at_seconds":60.9,
        "affected_components":[{"component_id":"api","component_name":"API"}],
        "updates":[{"at_seconds":30,"status":"investigating","description":"处理中"}],
    });
    let raw = json!({"nested":[{"initialPageConfig":{"components":[
        {"component_id":"api","name":"API","description":"Developer API"},
        {"component_id":"web","name":"Web"},
    ]},"active_changes":[incident.clone(),{"status":"maintenance","affected_components":[{"component_id":"web"}]}],
    "component_uptimes":[{"component_id":"api","uptime":99.95}],
    "initialCalendarData":{"changes":[incident]}}]});
    let encoded = serde_json::to_string(&format!("1:{raw}")).unwrap();
    let html = format!("<script>self.__next_f.push([1,{encoded}])</script>");
    let host = Arc::new(FixtureHost::new(&[(200, &html), (200, &html)]));
    let plugin = runtime("deepseek-status", host);
    let text = plugin
        .call_tool(
            "query_deepseek_status",
            json!({}),
            InvocationContext::default(),
        )
        .await
        .unwrap();
    let mut response: Value = serde_json::from_str(&text).unwrap();
    let queried_at = response
        .as_object_mut()
        .unwrap()
        .remove("queried_at")
        .unwrap();
    assert!(queried_at.as_str().unwrap().ends_with("+08:00"));
    assert_eq!(
        response,
        json!({
            "success":true, "overall_status":"partial_outage", "status_readable":"Partial Outage - 2 active incident(s)",
            "active_incidents_count":2,
            "components":[
                {"id":"api","name":"API","description":"Developer API","status":"partial_outage","uptime_30d_percent":99.95},
                {"id":"web","name":"Web","description":"","status":"maintenance","uptime_30d_percent":null},
            ],
            "recent_incidents":[{
                "id":"incident-1", "title":"API \"问题\"", "type":"incident", "status":"partial_outage",
                "started_at":"1970-01-01T08:00:00+08:00", "resolved_at":"1970-01-01T08:01:00+08:00", "duration_seconds":60,
                "affected_components":["API"],
                "timeline":[{"time":"1970-01-01T08:00:30+08:00","status":"investigating","description":"处理中"}],
            }],
        })
    );
    let response: Value = serde_json::from_str(
        &plugin
            .call_tool(
                "query_deepseek_status",
                json!({"include_incidents":false}),
                InvocationContext::default(),
            )
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(response["recent_incidents"], json!([]));
}

/// 【插件测试】【空状态】可选字段为空时输出兼容空数组，HTML 缺少有效载荷时明确失败。
#[tokio::test]
async fn deepseek_empty_data_and_invalid_pages_are_distinguished() {
    let encoded = serde_json::to_string("1:{\"initialPageConfig\":{\"components\":[]}}").unwrap();
    let html = format!("self.__next_f.push([1,{encoded}])");
    let plugin = runtime(
        "deepseek-status",
        Arc::new(FixtureHost::new(&[
            (200, &html),
            (200, "<html>invalid</html>"),
        ])),
    );
    let response: Value = serde_json::from_str(
        &plugin
            .call_tool(
                "query_deepseek_status",
                json!({}),
                InvocationContext::default(),
            )
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(response["overall_status"], "operational");
    assert_eq!(response["components"], json!([]));
    assert_eq!(response["recent_incidents"], json!([]));
    assert!(plugin
        .call_tool(
            "query_deepseek_status",
            json!({}),
            InvocationContext::default()
        )
        .await
        .is_err());
}

/// 【插件测试】【入口一致性】真实共用和只读工具表都使用迁移后的 Lua 工具。
#[test]
fn migrated_tools_are_present_in_common_and_readonly_registries() {
    let root = tempfile::tempdir().unwrap();
    let paths = crate::paths::SaiPaths::for_tests(root.path());
    let mut config = crate::config::AppConfig::default();
    config.plugins.archlinux.enabled = true;
    config.plugins.man.enabled = true;
    let common = crate::tools::builtin_registry_without_mcp(&config, &paths);
    let readonly = crate::tools::readonly_registry(&config, &paths);
    for name in [
        "online_man_search",
        "online_man_get_page",
        "query_deepseek_status",
        "aur_search_packages",
        "aur_get_package_info",
        "archlinux_official_package_query",
        "aur_check_status",
        "archwiki_query",
        "fcitx5_input_method_wiki_qurey",
        "protondb_query",
        "web_search",
        "gather_linux_game_compatibility_signals",
        "linux_game_compatibility",
        "linux_input_method_diagnose",
        "check_issue",
    ] {
        assert!(common.contains(name));
        assert!(readonly.contains(name));
        assert!(common.plugin_owner(name).is_some());
        assert!(readonly.plugin_owner(name).is_some());
        assert_eq!(
            common.definition(name).unwrap().function.parameters,
            readonly.definition(name).unwrap().function.parameters
        );
    }
    assert!(common.plugin_diagnostics().is_empty());
    assert!(readonly.plugin_diagnostics().is_empty());
}

/// 【插件检查测试】【内置源码】所有内置源码包都必须通过实际管理检查入口，避免发布无法检查的工具名称。
#[test]
fn bundled_source_packages_pass_the_management_check() {
    for package in crate::plugins::bundled::packages().unwrap() {
        let id = &package.manifest.id;
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("plugins")
            .join(id);
        let inspected = crate::plugins::validate_package(&directory)
            .unwrap_or_else(|error| panic!("bundled package {id} cannot be checked: {error:#}"));
        assert_eq!(inspected.manifest.id, *id);
    }
}

/// 【插件测试】【目录兼容】内置工具禁用后仍可预先加入 Agent 白名单，但不能实际调用。
#[test]
fn disabled_bundled_tools_remain_configurable_without_becoming_callable() {
    let root = tempfile::tempdir().unwrap();
    let paths = crate::paths::SaiPaths::for_tests(root.path());
    let config = crate::config::AppConfig::default();
    for id in [
        "online-man",
        "archlinux",
        "fcitx-wiki",
        "protondb",
        "web-search",
    ] {
        crate::plugins::set_enabled(
            &config,
            &paths,
            id,
            false,
            crate::plugins::GrantUpdate::Keep,
        )
        .unwrap();
    }
    let actual = crate::tools::builtin_registry_without_mcp(&config, &paths);
    let catalog = crate::tools::tool_catalog(&config, &paths);
    for name in [
        "online_man_search",
        "aur_search_packages",
        "aur_get_package_info",
        "archlinux_official_package_query",
        "aur_check_status",
        "archwiki_query",
        "fcitx5_input_method_wiki_qurey",
        "protondb_query",
        "web_search",
    ] {
        assert!(!actual.contains(name));
        assert!(catalog.iter().any(|tool| tool.name == name));
    }
}

/// 【插件测试】【Arch 开关】旧配置决定缺省状态，显式插件设置覆盖旧开关。
#[test]
fn archlinux_plugin_settings_override_the_legacy_default_in_both_registries() {
    let root = tempfile::tempdir().unwrap();
    let paths = crate::paths::SaiPaths::for_tests(root.path());
    let mut config = crate::config::AppConfig::default();
    config.plugins.archlinux.enabled = false;
    let initial = crate::tools::builtin_registry_without_mcp(&config, &paths);
    assert!(!initial.contains("aur_search_packages"));
    assert!(initial.contains("fcitx5_input_method_wiki_qurey"));
    assert!(initial.contains("protondb_query"));
    for (enabled, legacy) in [(true, false), (false, true)] {
        config.plugins.archlinux.enabled = legacy;
        crate::plugins::set_enabled(
            &config,
            &paths,
            "archlinux",
            enabled,
            crate::plugins::GrantUpdate::Keep,
        )
        .unwrap();
        for registry in [
            crate::tools::builtin_registry_without_mcp(&config, &paths),
            crate::tools::readonly_registry(&config, &paths),
        ] {
            for name in [
                "aur_search_packages",
                "aur_get_package_info",
                "archlinux_official_package_query",
                "aur_check_status",
                "archwiki_query",
            ] {
                assert_eq!(registry.contains(name), enabled);
            }
        }
    }
}
