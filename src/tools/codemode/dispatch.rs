use anyhow::{bail, Result};

/// 【CodeMode】【网关边界】参数 name 为工具名，返回是否为不应嵌套的递归或发现网关。
pub(crate) fn is_gateway(name: &str) -> bool {
    name == super::NAME
        || [
            crate::tools::LOAD_NAME,
            crate::tools::INVOKE_NAME,
            crate::tools::REQUEST_CAPABILITY_NAME,
        ]
        .contains(&name)
}

/// 【CodeMode】【结果转换】把 output 转为脚本值，返回 JSON、文本或包含图片的结果对象。
pub(crate) fn result_value(output: crate::tools::ToolOutput) -> Result<serde_json::Value> {
    if output.content.starts_with("tool error:") {
        bail!("{}", output.content);
    }
    let value = serde_json::from_str::<serde_json::Value>(&output.content)
        .unwrap_or(serde_json::Value::String(output.content));
    if output.model_attachments.is_empty() {
        return Ok(value);
    }
    Ok(
        serde_json::json!({"result": value, "images": output.model_attachments.into_iter().map(|image| serde_json::json!({"image_url":image.image_url, "source":image.source, "note":image.note})).collect::<Vec<_>>()}),
    )
}
