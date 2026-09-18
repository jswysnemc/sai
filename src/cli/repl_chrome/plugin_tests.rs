use super::*;
use crate::paths::SaiPaths;
use crate::plugins::{self, GrantChanges, GrantUpdate, TuiStatusRenderer};
use serde_json::json;

/// 【底栏集成测试】【安装样本】通过公共安装、配置和授权入口准备真实 Lua 插件。
/// @param paths 隔离应用目录；返回默认主配置
fn install(paths: &SaiPaths) -> AppConfig {
    let config = AppConfig::default();
    plugins::install(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/lua-plugins/tui-status"),
        paths,
        false,
    )
    .unwrap();
    plugins::set_enabled(
        &config,
        paths,
        "tui-status",
        true,
        GrantUpdate::Changes(GrantChanges {
            tui_status: Some(true),
            ..Default::default()
        }),
    )
    .unwrap();
    config
}

/// 【底栏集成测试】【界面样本】构造实际底栏并绑定异步插件计算器。
/// @param config 主配置；paths 为隔离目录；返回可绘制底栏
fn chrome(config: &AppConfig, paths: &SaiPaths) -> ReplChrome {
    ReplChrome {
        mode: AgentMode::Yolo,
        context_ratio: 0.25,
        context_window_tokens: 128000,
        model: "test-model".into(),
        thinking: "high".into(),
        directory: "~/项目/sai".into(),
        cache_hit_ratio: Some(0.9),
        activity: None,
        role_badge: None,
        status_plugin: Some(TuiStatusRenderer::start(config.clone(), paths.clone())),
    }
}

/// 【底栏集成测试】【刷新等待】等待真实后台插件结果进入实际底栏渲染。
/// @param chrome 底栏；cols 为列数；expected 为预期文本；返回去掉颜色的底栏
async fn rendered(chrome: &ReplChrome, cols: usize, expected: &str) -> String {
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            let line =
                crate::cli::repl_text::strip_terminal_control_sequences(&chrome.footer_line(cols));
            if line.contains(expected) {
                return line;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap()
}

/// 【底栏集成测试】【实时布局】实际安装包控制字段，状态更新有效，活动和角色标记保持可见。
#[tokio::test]
async fn tui_status_plugin_renders_configuration_live_usage_and_narrow_terminals() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = install(&paths);
    let mut chrome = chrome(&config, &paths);
    let line = rendered(&chrome, 120, "yolo · test-model · 25.0%/128k · cache 90%").await;
    assert!(line.trim_end().ends_with("sai"));
    assert!(!line.contains("high") && !line.contains("~/项目"));
    chrome.set_mode(AgentMode::Plan);
    chrome.apply_live_usage(Some(64000), Some(0.5));
    chrome.set_role_badge(Some("跟随中".into()));
    chrome.set_activity(Some("Ctrl+C 停止".into()));
    let line = rendered(&chrome, 120, "plan · test-model · 50.0%/128k · cache 50%").await;
    assert!(line.trim_start().starts_with("跟随中  Ctrl+C 停止"));
    let narrow = rendered(&chrome, 50, "plan · test-model").await;
    assert!(!narrow.contains("cache"));
    assert!(visible_width(&narrow) <= 50);
    assert!(chrome.status_diagnostics().is_empty());
}

/// 【底栏集成测试】【配置与撤销】公共配置支持字段左右交换，非法配置不落盘，撤销恢复默认。
#[tokio::test]
async fn tui_status_plugin_configuration_and_grants_follow_public_management() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = install(&paths);
    plugins::configure(
        &config,
        &paths,
        "tui-status",
        json!({"left":["label","thinking"],"right":["model"],"label":"自定义", "separator":" / "}),
    )
    .unwrap();
    let line = rendered(&chrome(&config, &paths), 80, "自定义 / high").await;
    assert!(line.trim_end().ends_with("test-model") && !line.contains("25.0%"));
    let before = std::fs::read(paths.config_dir.join("plugins.jsonc")).unwrap();
    for settings in [
        json!({"left":["typo"]}),
        json!({"left":["model","model"]}),
        json!({"right":"model"}),
        json!({"separator":"\n"}),
        json!({"compact_below":-1}),
        json!({"unknown":true}),
        json!({"separator":false}),
        json!({"directory_style":false}),
    ] {
        assert!(plugins::configure(&config, &paths, "tui-status", settings).is_err());
        assert_eq!(
            std::fs::read(paths.config_dir.join("plugins.jsonc")).unwrap(),
            before
        );
    }
    plugins::set_enabled(
        &config,
        &paths,
        "tui-status",
        true,
        GrantUpdate::Changes(GrantChanges {
            tui_status: Some(false),
            ..Default::default()
        }),
    )
    .unwrap();
    let chrome = chrome(&config, &paths);
    let renderer = chrome.status_plugin.as_ref().unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        while renderer.active() {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let line = rendered(&chrome, 80, "high").await;
    assert!(!line.contains("自定义"));
    assert!(line.contains("25.0%"));
}

/// 【底栏集成测试】【授权参数】底栏开关独立于通知，冲突的命令必须拒绝。
#[test]
fn tui_status_cli_grants_are_independent() {
    use clap::Parser;
    assert!(crate::cli::Cli::try_parse_from([
        "sai",
        "plugins",
        "enable",
        "tui-status",
        "--allow-tui-status",
        "--no-notifications"
    ])
    .is_ok());
    for flags in [
        ["--allow-tui-status", "--no-tui-status"],
        ["--grant-declared", "--no-tui-status"],
        ["--grant-declared", "--allow-tui-status"],
    ] {
        let mut args = vec!["sai", "plugins", "enable", "tui-status"];
        args.extend(flags);
        assert!(crate::cli::Cli::try_parse_from(args).is_err());
    }
}

/// 【底栏集成测试】【故障回退】失控回调在后台终止，只报告一次错误并保持原生布局。
#[tokio::test]
async fn tui_status_plugin_failure_falls_back_without_blocking_input() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = install(&paths);
    std::fs::write(
        paths.config_dir.join("plugins/tui-status/init.lua"),
        "sai.on('tui_status', function() while true do end end)",
    )
    .unwrap();
    let chrome = chrome(&config, &paths);
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        assert!(chrome.footer_line(120).contains("test-model"));
    }
    assert!(start.elapsed() < std::time::Duration::from_secs(1));
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        while chrome.status_plugin.as_ref().unwrap().active() {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let diagnostics = chrome.status_diagnostics();
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].contains("tui-status"));
    assert!(chrome.status_diagnostics().is_empty());
    assert!(chrome.footer_line(120).contains("test-model"));
}

/// 【底栏集成测试】【快照缓存】重复绘制不重调 Lua，变化后不能显示上一个模式的缓存。
#[tokio::test]
async fn tui_status_plugin_caches_identical_frames_and_rejects_stale_results() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = install(&paths);
    std::fs::write(paths.config_dir.join("plugins/tui-status/init.lua"),
        "local calls=0; sai.on('tui_status', function(event) calls=calls+1; return {left='Lua:'..event.mode..':'..calls,right=''} end)").unwrap();
    let mut chrome = chrome(&config, &paths);
    rendered(&chrome, 80, "Lua:yolo:1").await;
    for _ in 0..100 {
        assert!(chrome.footer_line(80).contains("Lua:yolo:1"));
    }
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    assert!(chrome.footer_line(80).contains("Lua:yolo:1"));
    chrome.set_mode(AgentMode::Plan);
    assert!(!chrome.footer_line(80).contains("Lua:yolo"));
    rendered(&chrome, 80, "Lua:plan:2").await;
}
