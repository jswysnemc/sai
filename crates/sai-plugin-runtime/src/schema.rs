use anyhow::{bail, Context, Result};
use serde_json::Value;

/// 【插件】【参数契约】编译仅引用本地定义的对象 Schema，供工具和进程模板共同使用。
/// @param schema 有界的 JSON Schema
/// @returns 可重复使用的参数校验器
pub(crate) fn compile_object(schema: &Value) -> Result<jsonschema::Validator> {
    if schema.get("type").and_then(Value::as_str) != Some("object") {
        bail!("plugin parameters must use an object JSON Schema");
    }
    if serde_json::to_vec(schema)?.len() > 64 * 1024 {
        bail!("plugin schema exceeds 64 KiB");
    }
    validate_local_refs(schema)?;
    jsonschema::validator_for(schema).context("invalid plugin parameter schema")
}

/// 【插件】【本地引用】拒绝 Schema 中的外部文件和网络引用。
/// @param value 当前 Schema 节点
/// @returns 全部引用均限制在当前文档时成功
fn validate_local_refs(value: &Value) -> Result<()> {
    match value {
        Value::Object(fields) => {
            for (name, value) in fields {
                if matches!(name.as_str(), "$ref" | "$dynamicRef" | "$recursiveRef")
                    && value
                        .as_str()
                        .is_some_and(|reference| !reference.starts_with('#'))
                {
                    bail!("plugin schemas may only reference local definitions");
                }
                validate_local_refs(value)?;
            }
        }
        Value::Array(items) => {
            for item in items {
                validate_local_refs(item)?;
            }
        }
        _ => {}
    }
    Ok(())
}
