use super::image_support::*;
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::plugins::discovery::find;
use crate::plugins::registry::register_descriptor;
use crate::tools::ToolRegistry;
use serde_json::{json, Value};
use std::sync::Arc;

/// 【图片显示测试】【真实工具】语言、路径修剪和终端百分比通过实际 Lua 工具生效。
#[tokio::test]
async fn display_tool_uses_its_settings_and_returns_localized_results() {
    let host = Arc::new(ImageHost::default());
    *host.terminal.lock().unwrap() = Some((120, 40));
    for (language, message) in [
        ("en", "printed image in terminal"),
        ("zh", "已在终端打印图片"),
    ] {
        let plugin = runtime(
            "image-display",
            json!({"language":language,"width_percent":25,"height_percent":20}),
            host.clone(),
        );
        let result = plugin
            .call_tool(
                "print_image",
                json!({"image":" picture.png "}),
                Default::default(),
            )
            .await
            .unwrap();
        assert_eq!(result, format!("{message}: picture.png"));
        assert_eq!(
            host.displays.lock().unwrap().last().unwrap(),
            &("picture.png".into(), Some("30x8".into()))
        );
        let error = plugin
            .call_tool("print_image", json!({"image":" "}), Default::default())
            .await
            .unwrap_err();
        assert!(format!("{error:#}").contains(if language == "zh" {
            "缺少图片路径"
        } else {
            "image is required"
        }));
    }
}

/// 【图片显示测试】【真实插件组合】生成包经正式工具服务调用显示包，其设置与授权独立生效。
#[tokio::test]
async fn generation_composes_with_the_actual_display_plugin_through_tool_services() {
    for (enabled, authorized) in [(true, true), (true, false), (false, true)] {
        let root = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(root.path());
        let mut config = AppConfig::default();
        config.plugins.image_generation.auto_print = true;
        config.plugins.image_generation.api_keys = vec!["fixture-key".into()];
        config.plugins.image_generation.output_dir = "output".into();
        config.plugins.print_image.width_percent = 20;
        config.plugins.print_image.height_percent = 25;
        let host = Arc::new(ImageHost::new(vec![(
            200,
            br#"{"data":[{"b64_json":"aGVsbG8="}]}"#.to_vec(),
        )]));
        *host.terminal.lock().unwrap() = Some((120, 40));
        let mut registry = ToolRegistry::new();
        register_descriptor(
            &mut registry,
            find(&config, &paths, "image-generation").unwrap(),
            host.clone(),
            false,
        )
        .unwrap();
        if enabled {
            let mut descriptor = find(&config, &paths, "image-display").unwrap();
            if !authorized {
                descriptor.setting.grants = Some(Default::default());
            }
            register_descriptor(&mut registry, descriptor, host.clone(), false).unwrap();
        }
        let result = registry
            .call("generate_image", &json!({"prompt":"preview"}).to_string())
            .await
            .unwrap();
        let result: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(result["status"], "ok");
        assert_eq!(result["printed"], enabled && authorized);
        assert_eq!(host.writes.lock().unwrap().len(), 1);
        if enabled && authorized {
            let displays = host.displays.lock().unwrap();
            assert_eq!(displays.len(), 1);
            assert_eq!(displays[0].1, Some("24x10".into()));
        } else {
            assert!(host.displays.lock().unwrap().is_empty());
        }
        if enabled && !authorized {
            assert!(result["print_error"]
                .as_str()
                .unwrap()
                .contains("not allowed"));
        }
    }
}
