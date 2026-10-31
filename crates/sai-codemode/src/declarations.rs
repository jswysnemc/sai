use serde_json::Value;

/// 【CodeMode】【类型生成】从 schema 生成有深度上限的 TypeScript 类型，返回类型文本。
pub fn schema_type(schema: &Value, depth: usize) -> String {
    if depth > 8 {
        return "unknown".into();
    }
    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        return values
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join(" | ");
    }
    for union in ["anyOf", "oneOf"] {
        if let Some(items) = schema.get(union).and_then(Value::as_array) {
            return items
                .iter()
                .map(|v| schema_type(v, depth + 1))
                .collect::<Vec<_>>()
                .join(" | ");
        }
    }
    match schema.get("type").and_then(Value::as_str) {
        Some("string") => "string".into(),
        Some("number" | "integer") => "number".into(),
        Some("boolean") => "boolean".into(),
        Some("null") => "null".into(),
        Some("array") => format!("Array<{}>", schema_type(&schema["items"], depth + 1)),
        Some("object") => {
            let mut fields = Vec::new();
            if let Some(properties) = schema["properties"].as_object() {
                for (name, value) in properties {
                    let required = schema["required"]
                        .as_array()
                        .is_some_and(|items| items.iter().any(|item| item.as_str() == Some(name)));
                    fields.push(format!(
                        "{}{}: {}",
                        serde_json::to_string(name).unwrap(),
                        if required { "" } else { "?" },
                        schema_type(value, depth + 1)
                    ));
                }
            }
            if schema["additionalProperties"] != false {
                fields.push("[key: string]: unknown".into());
            }
            format!("{{ {} }}", fields.join("; "))
        }
        _ => "unknown".into(),
    }
}

/// 【CodeMode】【声明生成】根据 name/schema 生成工具声明，返回可供提示词使用的文本。
pub fn tool_declaration(name: &str, schema: &Value) -> String {
    format!(
        "{}(args: {}): Promise<unknown>;",
        serde_json::to_string(name).unwrap(),
        schema_type(schema, 0)
    )
}
