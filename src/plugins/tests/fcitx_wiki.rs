use super::support::{call_json, runtime, FixtureHost};
use sai_plugin_runtime::InvocationContext;
use serde_json::{json, Value};
use std::sync::Arc;

const TOOL: &str = "fcitx5_input_method_wiki_qurey";

/// 【插件测试】【主题优先级】混合问题沿用原主题顺序，关闭摘录后不访问网络。
#[tokio::test]
async fn fcitx_auto_selection_preserves_priority_without_network() {
    let host = Arc::new(FixtureHost::default());
    let plugin = runtime("fcitx-wiki", host.clone());
    for (query, topic) in [
        ("wechat XWayland XMODIFIERS", "xim"),
        ("Electron GTK_IM_MODULE", "electron_chromium"),
        ("GTK Qt Wayland", "gtk"),
        ("QT_IM_MODULES LC_CTYPE", "qt"),
        ("LC_CTYPE Wayland", "locale"),
        ("Wayland setup", "wayland"),
        ("输入法配置", "setup"),
        ("general question", "for_users"),
    ] {
        let result = call_json(
            &plugin,
            TOOL,
            json!({
                "query":query, "include_page_excerpt":false,
            }),
        )
        .await;
        assert_eq!(result["主题"], topic);
        assert_eq!(result["页面摘录"], Value::Null);
    }
    assert!(host.requests.lock().unwrap().is_empty());
}

/// 【插件测试】【中英文包装】默认双语、纯中文和英文结构均保持旧字段。
#[tokio::test]
async fn fcitx_language_wrappers_keep_claims_and_optional_fields() {
    let plugin = runtime("fcitx-wiki", Arc::new(FixtureHost::default()));
    let bilingual = call_json(
        &plugin,
        TOOL,
        json!({
            "topic":"electron_chromium", "include_page_excerpt":false,
        }),
    )
    .await;
    assert_eq!(bilingual["工具"], TOOL);
    assert_eq!(bilingual["查询"], Value::Null);
    assert_eq!(bilingual["来源"]["标题"], "Using Fcitx 5 on Wayland");
    assert!(bilingual["官方规则摘录"][0]["结论"]
        .as_str()
        .unwrap()
        .contains("不要断言"));
    assert!(bilingual["官方规则摘录"][0]["english_reference"]
        .as_str()
        .unwrap()
        .contains("do not assert"));
    assert!(bilingual["diagnostic_rule_en"]
        .as_str()
        .unwrap()
        .contains("Local runtime evidence"));

    let zh = call_json(
        &plugin,
        TOOL,
        json!({
            "topic":"xim", "language":"zh", "include_page_excerpt":false,
        }),
    )
    .await;
    assert!(zh.get("diagnostic_rule_en").is_none());
    for claim in zh["官方规则摘录"].as_array().unwrap() {
        assert!(claim.get("english_reference").is_none());
        assert_eq!(claim["可信度"], "official_wiki_general_rule");
    }
    let en = call_json(&plugin, TOOL, json!({
            "query":"　introduction  ", "topic":"home", "language":"en", "include_page_excerpt":false,
    })).await;
    assert_eq!(
        en,
        json!({
            "ok":true, "tool":TOOL,
            "spelling_note":"Tool name keeps the requested 'qurey' spelling for compatibility.",
            "topic":"home", "query":"introduction",
            "source":{"title":"Fcitx 5","url":"https://fcitx-im.org/wiki/Special:MyLanguage/Fcitx_5"},
            "claims":[{
                "claim":{"en":"Fcitx 5 is an extensible input method framework; the home page's For Users section links to install, setup, FAQ, Wayland usage, tips, and upgrade pages."},
                "applies_to":"orientation", "confidence":"official_wiki_general_rule",
                "caveat":"Use the home page to discover official user-facing pages, not to diagnose one app by itself.",
            }],
            "diagnostic_rule":{"en":"Local runtime evidence comes first; the Wiki provides official general rules and must not override /proc data, environment, loaded modules, or an actual input test."},
            "page_excerpt":null,
        })
    );
}

/// 【插件测试】【静态主题】每个公开主题保留规则数量与固定来源，环境变量空规则保持数组。
#[tokio::test]
async fn fcitx_topics_keep_the_existing_rule_sets_and_whitelist() {
    let plugin = runtime("fcitx-wiki", Arc::new(FixtureHost::default()));
    for (topic, count, path) in [
        ("home", 1, "Fcitx_5"),
        ("for_users", 1, "Fcitx_5"),
        ("setup", 2, "Setup_Fcitx_5"),
        ("wayland", 3, "Using_Fcitx_5_on_Wayland"),
        ("electron_chromium", 2, "Using_Fcitx_5_on_Wayland"),
        (
            "environment_variables",
            0,
            "Input_method_related_environment_variables",
        ),
        ("xim", 3, "Input_method_related_environment_variables"),
        ("gtk", 2, "Input_method_related_environment_variables"),
        ("qt", 2, "Input_method_related_environment_variables"),
        ("locale", 2, "Input_method_related_environment_variables"),
    ] {
        let result = call_json(
            &plugin,
            TOOL,
            json!({
                "topic":topic, "language":"en", "include_page_excerpt":false,
            }),
        )
        .await;
        assert_eq!(result["claims"].as_array().unwrap().len(), count);
        assert_eq!(
            result["source"]["url"],
            format!("https://fcitx-im.org/wiki/Special:MyLanguage/{path}")
        );
    }
}

/// 【插件测试】【页面正文】完整提取嵌套 div，保留中文和链接，排除导航与页脚。
#[tokio::test]
async fn fcitx_excerpt_extracts_the_whole_nested_wiki_body() {
    let html = r#"<script>var marker = "mw-parser-output";</script><nav>navigation</nav>
        <div class='mw-parser-output extra'><p>开头</p><div><p>嵌套正文</p></div>
        <div><div><a href="/wiki/Setup_Fcitx_5">安装指南</a></div></div><p>最后正文</p></div>
        <footer>footer text</footer>"#;
    let host = Arc::new(FixtureHost::new(&[(200, html)]));
    let plugin = runtime("fcitx-wiki", host.clone());
    let result = call_json(&plugin, TOOL, json!({"topic":"xim"})).await;
    let excerpt = result["页面摘录"].as_str().unwrap();
    assert!(excerpt.contains("开头"));
    assert!(excerpt.contains("嵌套正文"));
    assert!(excerpt.contains("[安装指南](/wiki/Setup_Fcitx_5)"));
    assert!(excerpt.contains("最后正文"));
    assert!(!excerpt.contains("navigation"));
    assert!(!excerpt.contains("footer text"));
    assert!(!excerpt.contains("var marker"));
    let requests = host.requests.lock().unwrap();
    assert_eq!(
        requests[0].url,
        "https://fcitx-im.org/wiki/Special:MyLanguage/Input_method_related_environment_variables"
    );
    assert_eq!(requests[0].max_bytes, 512 * 1024);
    assert_eq!(requests[0].timeout_ms, 12000);
    assert_eq!(requests[0].method, "GET");
}

/// 【插件测试】【摘录长度】按 Unicode 字符截断，恰好达到上限时不增加说明。
#[tokio::test]
async fn fcitx_excerpt_clips_unicode_at_the_legacy_character_limit() {
    let long = format!(
        "<div class=\"mw-parser-output\"><p>{}</p></div>",
        "界".repeat(12001)
    );
    let exact = format!("<p>{}</p>", "界".repeat(12000));
    let host = Arc::new(FixtureHost::new(&[(200, &long), (200, &exact)]));
    let plugin = runtime("fcitx-wiki", host);
    let clipped = call_json(&plugin, TOOL, json!({"topic":"home", "language":"en"})).await;
    assert!(
        clipped["page_excerpt"] == format!("{}\n...[truncated to 12000 chars]", "界".repeat(12000))
    );
    let exact = call_json(&plugin, TOOL, json!({"topic":"home", "language":"en"})).await;
    assert!(exact["page_excerpt"] == "界".repeat(12000));
}

/// 【插件测试】【摘录失败】HTTP、体积和连接失败均保留静态规则。
#[tokio::test]
async fn fcitx_excerpt_failure_keeps_the_rules_available() {
    let oversized = "x".repeat(512 * 1024 + 1);
    let host = Arc::new(FixtureHost::new(&[(503, "unavailable"), (200, &oversized)]));
    let plugin = runtime("fcitx-wiki", host);
    for marker in ["HTTP 503", "byte limit", "no HTTP fixture"] {
        let result = call_json(&plugin, TOOL, json!({"topic":"xim", "language":"en"})).await;
        assert_eq!(result["ok"], true);
        assert_eq!(result["claims"].as_array().unwrap().len(), 3);
        let excerpt = result["page_excerpt"].as_str().unwrap();
        assert!(excerpt.starts_with("[fetch failed:"));
        assert!(excerpt.contains(marker), "{excerpt}");
    }
}

/// 【插件测试】【主题校验】非法主题、语言和额外字段不能触发网络访问。
#[tokio::test]
async fn fcitx_rejects_arguments_outside_the_public_schema() {
    let host = Arc::new(FixtureHost::default());
    let plugin = runtime("fcitx-wiki", host.clone());
    for args in [
        json!({"topic":"arbitrary"}),
        json!({"language":"fr"}),
        json!({"url":"https://unrelated.test"}),
    ] {
        assert!(plugin
            .call_tool(TOOL, args, InvocationContext::default())
            .await
            .is_err());
    }
    assert!(host.requests.lock().unwrap().is_empty());
}

/// 【插件测试】【真实页面】使用实际宿主验证页面正文和预期术语。
#[tokio::test]
#[ignore = "requires external network access to fcitx-im.org"]
async fn fcitx_live_excerpt_returns_official_content() {
    let plugin = runtime("fcitx-wiki", Arc::new(crate::plugins::host::SaiPluginHost));
    let result = call_json(&plugin, TOOL, json!({"topic":"xim", "language":"en"})).await;
    let text = result["page_excerpt"].as_str().unwrap();
    assert!(!text.starts_with("[fetch failed:"), "{text}");
    assert!(text.chars().count() > 100);
    let lower = text.to_ascii_lowercase();
    assert!(
        lower.contains("xmodifiers")
            || lower.contains("gtk_im_module")
            || lower.contains("qt_im_module")
    );
    assert!(!lower.contains("rlconf"));
}

/// 【插件测试】【真实双语】实际页面读取接入中文包装，避免只覆盖纯静态规则。
#[tokio::test]
#[ignore = "requires external network access to fcitx-im.org"]
async fn fcitx_live_bilingual_query_contains_an_excerpt() {
    let plugin = runtime("fcitx-wiki", Arc::new(crate::plugins::host::SaiPluginHost));
    let result = call_json(
        &plugin,
        TOOL,
        json!({"topic":"xim", "include_page_excerpt":true}),
    )
    .await;
    let text = result["页面摘录"].as_str().unwrap();
    assert!(!text.starts_with("[fetch failed:"), "{text}");
    assert!(!text.is_empty());
    assert!(!result["官方规则摘录"].as_array().unwrap().is_empty());
}
