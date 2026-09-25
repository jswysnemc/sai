mod common;

use common::runtime;
use sai_plugin_runtime::InvocationContext;
use serde_json::Value;

/// 【宿主功能测试】【初始化探测】加载期可以查询本构建有哪些接口，未实现的名字返回 false。
#[tokio::test]
async fn host_feature_query_is_available_before_any_capability_grant() {
    let plugin = runtime(
        r#"
        local info = sai.host.info()
        assert(info.api_version == 1, "api version")
        assert(sai.host.has("json") == true)
        assert(sai.host.has("sqlite") == true)
        assert(sai.host.has("session_history") == false)
        assert(sai.host.has("") == false)
        assert(not pcall(sai.host.has, string.rep("x", 65)))
        if sai.host.has("json") then
            sai.register_tool({
                name = "probe",
                description = "Return the host feature catalog.",
                parameters = { type = "object", properties = {}, additionalProperties = false },
                access = "read_only",
                execute = function()
                    return sai.json.encode(sai.host.info())
                end,
            })
        end
        "#,
    );
    assert!(plugin.tools().iter().any(|tool| tool.name == "probe"));
    let text = plugin
        .call_tool("probe", serde_json::json!({}), InvocationContext::default())
        .await
        .unwrap();
    let info: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(info["api_version"], 1);
    let features = info["features"].as_array().unwrap();
    assert!(features.iter().any(|feature| feature == "sqlite"));
    assert!(features.iter().all(|feature| feature != "session_history"));
    let names: Vec<&str> = features.iter().filter_map(Value::as_str).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted);
}
