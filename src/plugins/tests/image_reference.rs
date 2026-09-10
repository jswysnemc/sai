use super::image_support::*;
use sai_plugin_runtime::{InvocationContext, PluginPackage, PluginRuntime};
use serde_json::{json, Value};
use std::sync::Arc;

/// 【图片对照测试】【原版纯函数】固定原提交产生的请求、尺寸、命名和错误摘要必须与 Lua 一致。
#[tokio::test]
async fn lua_image_policy_matches_the_original_rust_reference() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/image_reference.json")).unwrap();
    assert_eq!(
        fixture["source_commit"],
        "1d104bef745dcd24be0ae9d77d10029f4dc663b0"
    );
    let packages = crate::plugins::bundled::packages().unwrap();
    let generation = packages
        .iter()
        .find(|p| p.manifest.id == "image-generation")
        .unwrap()
        .clone();
    let mut sources = generation.sources().clone();
    sources.insert("init.lua".into(),r#"
        local request=require('request'); local output=require('output')
        sai.register_tool({name='compare',description='Reference',parameters={type='object'},execute=function(args)
            if args.kind=='slug' then return output.slug(args.prompt) end
            if args.kind=='preview' then return request.preview(args.text,500) end
            return request.payload(args,'test prompt',args.ratio,args.resolution)
        end})
    "#.into());
    let generation = PluginPackage::new(generation.manifest, sources).unwrap();
    let display = packages
        .iter()
        .find(|p| p.manifest.id == "image-display")
        .unwrap()
        .clone();
    let mut sources = display.sources().clone();
    sources.insert("init.lua".into(),r#"
        local settings=require('settings')
        sai.register_tool({name='compare',description='Reference',parameters={type='object'},execute=function(args)
            local terminal=type(args.terminal)=='table' and args.terminal or nil
            return settings.size(args.args,args,terminal) or sai.json.null
        end})
    "#.into());
    let display = PluginPackage::new(display.manifest, sources).unwrap();
    let generation = PluginRuntime::load(
        generation,
        json!({}),
        Default::default(),
        Arc::new(ImageHost::default()),
    )
    .unwrap();
    let display = PluginRuntime::load(
        display,
        json!({}),
        Default::default(),
        Arc::new(ImageHost::default()),
    )
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let plugin = if case["kind"] == "size" {
            &display
        } else {
            &generation
        };
        let result = plugin
            .call_tool("compare", case.clone(), InvocationContext::default())
            .await
            .unwrap();
        let expected = if case["kind"] == "request" {
            &case["payload"]
        } else {
            &case["expected"]
        };
        let actual = if expected.is_object() {
            serde_json::from_str(&result).unwrap()
        } else if expected.is_null() {
            assert!(result.is_empty());
            Value::Null
        } else {
            Value::String(result)
        };
        assert_eq!(actual, *expected, "{case}");
    }
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 293);
}
