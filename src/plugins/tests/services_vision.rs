use super::services_support::{register_service, service_descriptor, ModelFixture, ModelReply};
use super::support::FixtureHost;
use crate::{paths::SaiPaths, plugins::registry::register_descriptor, tools::ToolRegistry};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;

const SOURCE: &str = r#"
sai.register_tool({name='run',description='Inspect image',parameters={type='object'},execute=function(args)
    if args.info then return sai.vision.info() end
    local text
    if args.text then text=sai.model.complete({messages={{role='user',content='text request'}}}).content end
    local image=sai.binary.decode_base64('YWJj')
    local response=image:analyze_image({system='Inspect only',prompt='Describe image',mime_type='image/png'})
    return {vision=response,text=text}
end})
"#;

/// 【视觉宿主测试】【独立模型】视觉供应商和模型覆盖保持旧规则，文本热切换及注册表过滤不丢失视觉绑定。
/// @returns 无；真实本地 HTTP 请求分别抵达选定的服务
#[tokio::test]
async fn vision_configuration_survives_text_model_switches_and_registry_filters() {
    let vision = ModelFixture::start(vec![
        ModelReply::text("vision one"),
        ModelReply::text("vision two"),
    ])
    .await;
    let text = ModelFixture::start(vec![
        ModelReply::text("text one"),
        ModelReply::text("text two"),
    ])
    .await;
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = text.config("configured-text");
    let mut provider = vision.config("vision-default").providers.remove(0);
    provider.id = "dedicated-vision".into();
    config.providers.push(provider);
    config.plugins.vision.enabled = true;
    config.plugins.vision.vision_provider_id = " dedicated-vision ".into();
    config.plugins.vision.vision_model = " configured-vision ".into();
    let mut tools = ToolRegistry::new();
    tools.configure_plugin_model(&config, &paths);
    register_service(
        &mut tools,
        "vision",
        SOURCE,
        json!({"vision":true,"model":true}),
    );
    let info: Value = serde_json::from_str(
        &tools
            .call("lua__vision__run", r#"{"info":true}"#)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        info,
        json!({"provider_id":"dedicated-vision","model":"configured-vision"})
    );
    assert!(text.requests().is_empty() && vision.requests().is_empty());
    for (model, expected_text, expected_vision) in [
        ("selected-agent", "text one", "vision one"),
        ("switched-agent", "text two", "vision two"),
    ] {
        tools.set_plugin_model_client(&text.client(model, &paths));
        let filtered = tools
            .clone_filtered(&["lua__vision__run"])
            .clone_excluding(&["unused"]);
        let result: Value = serde_json::from_str(
            &filtered
                .call("lua__vision__run", r#"{"text":true}"#)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(result["vision"]["content"], expected_vision);
        assert_eq!(result["vision"]["model"], "configured-vision");
        assert_eq!(result["vision"]["provider_id"], "dedicated-vision");
        assert_eq!(result["text"], expected_text);
    }
    assert_eq!(
        text.requests()
            .iter()
            .map(|request| request["model"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["selected-agent", "switched-agent"]
    );
    for request in vision.requests() {
        assert_eq!(request["model"], "configured-vision");
        assert!(request
            .get("tools")
            .is_none_or(|tools| tools.as_array().is_some_and(Vec::is_empty)));
        assert_eq!(request["messages"][0]["content"], "Inspect only");
        assert_eq!(request["messages"][1]["role"], "user");
        let content = request["messages"][1]["content"].as_array().unwrap();
        assert!(content.iter().any(|part| part["text"] == "Describe image"));
        assert!(content
            .iter()
            .any(|part| part["image_url"]["url"] == "data:image/png;base64,YWJj"));
    }
}

/// 【视觉宿主测试】【旧配置优先级】未设置视觉供应商时采用主配置供应商，模型覆盖只改变视觉请求。
/// @returns 无；当前文本客户端不参与视觉配置解析
#[tokio::test]
async fn empty_vision_overrides_use_the_configured_provider_and_model() {
    for override_model in ["", "vision-override"] {
        let fixture = ModelFixture::start(vec![ModelReply::text("image description")]).await;
        let root = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(root.path());
        let mut config = fixture.config("configured-default");
        config.plugins.vision.enabled = true;
        config.plugins.vision.vision_provider_id.clear();
        config.plugins.vision.vision_model = override_model.into();
        let mut tools = ToolRegistry::new();
        tools.configure_plugin_model(&config, &paths);
        tools.set_plugin_model_client(&fixture.client("selected-text", &paths));
        register_service(&mut tools, "vision", SOURCE, json!({"vision":true}));
        let result: Value =
            serde_json::from_str(&tools.call("lua__vision__run", "{}").await.unwrap()).unwrap();
        let expected = if override_model.is_empty() {
            "configured-default"
        } else {
            override_model
        };
        assert_eq!(result["vision"]["model"], expected);
        assert_eq!(fixture.requests()[0]["model"], expected);
    }
}

/// 【视觉宿主测试】【无可用配置】关闭和无效配置不能发送网络请求，错误不包含配置凭据。
/// @returns 无；禁用时返回空信息，配置错误可明确定位
#[tokio::test]
async fn disabled_or_invalid_vision_configuration_never_starts_a_request() {
    for invalid in [false, true] {
        let fixture = ModelFixture::start(vec![]).await;
        let root = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(root.path());
        let mut config = fixture.config("configured-default");
        config.plugins.vision.enabled = invalid;
        config.plugins.vision.vision_provider_id = "missing-provider".into();
        let mut tools = ToolRegistry::new();
        tools.configure_plugin_model(&config, &paths);
        register_service(&mut tools, "vision", SOURCE, json!({"vision":true}));
        let info = tools.call("lua__vision__run", r#"{"info":true}"#).await;
        if invalid {
            assert!(info.is_err());
        } else {
            assert_eq!(info.unwrap(), "");
        }
        let error = tools.call("lua__vision__run", "{}").await.unwrap_err();
        assert!(!format!("{error:#}").contains("fixture-only"));
        assert!(fixture.requests().is_empty());
    }
}

/// 【视觉宿主测试】【流式预算】供应商保持连接时，超长视觉正文也必须立即结束请求。
/// @returns 无；失败源于输出限制而非等待总超时
#[tokio::test]
async fn oversized_vision_streams_stop_before_the_provider_finishes() {
    let fixture = ModelFixture::start(vec![ModelReply::text(&"x".repeat(32768)).hold_open()]).await;
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = fixture.config("bounded-vision");
    config.plugins.vision.enabled = true;
    let mut tools = ToolRegistry::new();
    tools.configure_plugin_model(&config, &paths);
    let mut plugin = service_descriptor("vision", SOURCE, json!({"vision":true}));
    plugin.package.manifest.limits.output_bytes = 1024;
    register_descriptor(&mut tools, plugin, Arc::new(FixtureHost::default()), false).unwrap();
    let error = tokio::time::timeout(Duration::from_secs(2), tools.call("lua__vision__run", "{}"))
        .await
        .unwrap()
        .unwrap_err();
    assert!(format!("{error:#}").contains("response exceeds size limit"));
    assert_eq!(fixture.requests().len(), 1);
}
