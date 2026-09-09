mod common;

use common::system::capabilities;
use sai_plugin_runtime::{Capabilities, PluginManifest};
use serde_json::json;

/// 【系统授权测试】【完整模板】模板任一字段变化都会撤销原授权，路径与变量仍按精确交集保留。
#[test]
fn process_grants_are_bound_to_the_entire_template() {
    let declared = capabilities();
    for field in ["program", "args", "parameters", "read_only"] {
        let mut changed = serde_json::to_value(&declared).unwrap();
        changed["system"]["processes"]["read"][field] = match field {
            "program" => json!("other-fixture"),
            "args" => json!(["--different"]),
            "parameters" => {
                json!({"type":"object","properties":{"extra":{"type":"string"}},"additionalProperties":false})
            }
            _ => json!(false),
        };
        let changed: Capabilities = serde_json::from_value(changed).unwrap();
        changed.validate().unwrap();
        assert!(!changed.is_subset(&declared));
        let effective = changed.intersection(&declared);
        assert!(!effective.system.processes.contains_key("read"));
        assert!(effective.system.processes.contains_key("write"));
        assert_eq!(effective.system.read_paths, declared.system.read_paths);
        assert_eq!(effective.system.environment, declared.system.environment);
    }
}

/// 【系统授权测试】【参数契约】参数整体进入 argv，非法对象、控制字符及额外字段不能执行。
#[test]
fn parameters_expand_to_whole_arguments_and_writes_default_to_denied() {
    let grants: Capabilities = serde_json::from_value(json!({"system":{"processes":{"sample":{
        "program":"example", "args":["--",{"parameter":"value"}],
        "parameters":{"type":"object","properties":{"value":{"type":"string"}},"required":["value"],"additionalProperties":false}
    }}}})).unwrap();
    grants.validate().unwrap();
    let value = json!({"value":"a b; $(touch marker)"});
    assert!(grants
        .system
        .process_command("sample", &value, false)
        .is_err());
    assert_eq!(
        grants
            .system
            .process_command("sample", &value, true)
            .unwrap(),
        (
            "example".into(),
            vec!["--".into(), "a b; $(touch marker)".into()]
        )
    );
    for value in [
        json!({}),
        json!([]),
        json!({"value":{}}),
        json!({"value":"x","extra":true}),
        json!({"value":"x\u{0000}"}),
    ] {
        assert!(
            grants
                .system
                .process_command("sample", &value, true)
                .is_err(),
            "{value}"
        );
    }
}

/// 【系统授权测试】【清单拒绝】拒绝通配读取、外部 Schema 引用、未知参数和不受限参数对象。
#[test]
fn invalid_system_capabilities_fail_during_manifest_validation() {
    let baseline = serde_json::to_value(common::manifest()).unwrap();
    for system in [
        json!({"read_paths":["../secret"]}),
        json!({"read_paths":["/proc/*"]}),
        json!({"environment":["1BAD"]}),
        json!({"environment":["*"]}),
        json!({"processes":{"test":{"program":"-option"}}}),
        json!({"processes":{"test":{"program":"example","parameters":{"type":"object"}}}}),
        json!({"processes":{"test":{"program":"example","args":[{"parameter":"missing"}]}}}),
        json!({"processes":{"test":{"program":"example","parameters":{"type":"object","additionalProperties":false,"properties":{"x":{"$ref":"https://example.test/schema"}}}}}}),
    ] {
        let mut input = baseline.clone();
        input["capabilities"] = json!({"system":system});
        assert!(
            PluginManifest::parse(&input.to_string()).is_err(),
            "{input}"
        );
    }
    let mut input = baseline;
    input["limits"]["system_calls"] = json!(4097);
    assert!(PluginManifest::parse(&input.to_string()).is_err());
}
