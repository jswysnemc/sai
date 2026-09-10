use super::alarm_support::{self, AlarmHost};
use sai_plugin_runtime::{PluginPackage, PluginRuntime};
use serde_json::{json, Value};
use std::sync::Arc;

/// 【闹钟对照测试】【时间语义】固定原版结果覆盖时长、时钟、跨日及 Unicode 空白。
/// @returns 无，非法多字节单位必须返回错误而非使运行时崩溃
#[tokio::test]
async fn alarm_time_matches_frozen_native_results() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/alarm_reference.json")).unwrap();
    assert_eq!(fixture["source_commit"], "4ea97d5");
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 114);
    let package = alarm_support::package();
    let grants = package.manifest.capabilities.clone();
    let mut sources = package.sources().clone();
    sources.get_mut("init.lua").unwrap().push_str(r#"
        sai.register_tool({name="parse_time",description="Reference",parameters={type="object"},execute=function(args)
            local due=require("time").due_at(args.time,args.now)
            return {due_at=due,due_at_local=sai.time.local_format("%Y-%m-%d %H:%M:%S",due)}
        end})
    "#);
    let plugin = PluginRuntime::load(
        PluginPackage::new(package.manifest, sources).unwrap(),
        json!({}),
        grants,
        Arc::new(AlarmHost::default()),
    )
    .unwrap();
    for (index, case) in cases.iter().enumerate() {
        let result = alarm_support::call(
            &plugin,
            "parse_time",
            json!({"time":case["time"],"now":case["now"]}),
            false,
        )
        .await;
        if case["expected"].get("error").is_some() {
            assert!(result.is_err(), "case {index}: {case}");
        } else {
            assert_eq!(
                result.unwrap_or_else(|error| panic!("case {index}: {case}: {error:#}")),
                case["expected"],
                "case {index}"
            );
        }
    }
}
