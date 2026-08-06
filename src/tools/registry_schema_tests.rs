use super::{ToolRegistry, ToolSpec};

/// 验证注册表按照真实工具 Schema 拒绝缺失字段和错误类型。
#[test]
fn validates_arguments_against_registered_schema() {
    let mut registry = ToolRegistry::new();
    registry.register(ToolSpec::new(
        "lookup",
        "Lookup an item.",
        serde_json::json!({
            "type": "object",
            "properties": {"id": {"type": "integer"}},
            "required": ["id"],
            "additionalProperties": false
        }),
        |_| async { Ok("ok".to_string()) },
    ));

    assert!(registry.validate_arguments("lookup", r#"{"id":7}"#).is_ok());
    assert!(registry.validate_arguments("lookup", "{}").is_err());
    assert!(registry
        .validate_arguments("lookup", r#"{"id":"7"}"#)
        .is_err());
}
