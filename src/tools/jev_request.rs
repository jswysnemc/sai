use super::{ToolRegistry, ToolSpec};
use serde::Deserialize;
use serde_json::json;

/// 模型向 Jev 申请额外工具或 skill 的工具名。
pub(crate) const REQUEST_CAPABILITY_NAME: &str = "request_capability";

/// `request_capability` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CapabilityRequest {
    /// 用自然语言描述的能力需求
    pub need: String,
}

impl CapabilityRequest {
    /// 解析并校验工具参数。
    ///
    /// 参数:
    /// - `arguments`: 模型给出的 JSON 参数
    ///
    /// 返回:
    /// - 去除首尾空白后的需求；为空时报错
    pub(crate) fn parse(arguments: &str) -> anyhow::Result<Self> {
        let mut request: Self = serde_json::from_str(arguments)
            .map_err(|error| anyhow::anyhow!("invalid request_capability arguments: {error}"))?;
        request.need = request.need.trim().to_string();
        if request.need.is_empty() {
            anyhow::bail!("request_capability need must be non-empty");
        }
        Ok(request)
    }
}

/// 注册 Jev 能力申请工具；真实处理由对话运行时完成。
///
/// 参数:
/// - `registry`: 当前会话工具注册表
///
/// 返回:
/// - 无
pub(crate) fn register(registry: &mut ToolRegistry) {
    registry.register(ToolSpec::new(
            REQUEST_CAPABILITY_NAME,
            "Ask the Jev router for additional tools or skills. Only basic tools are exposed up front; everything else is exposed on demand. Describe in plain language what you need to do next (for example \"search the web for recent release notes\" or \"draw an architecture diagram\"). Jev picks matching tools and skills that are not exposed yet and returns their schemas or documents. Call exposed tools through invoke_tool. Resources already exposed in this conversation are never returned again; reuse the earlier result instead of asking twice.",
            json!({
                "type": "object",
                "properties": {
                    "need": {
                        "type": "string",
                        "minLength": 1,
                        "description": "Plain-language description of the capability needed for the next step."
                    }
                },
                "required": ["need"],
                "additionalProperties": false
            }),
            |_| async move { Ok("能力申请已收到。实际暴露结果由对话运行时生成。".to_string()) },
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_trims_need() {
        let request = CapabilityRequest::parse(r#"{"need":"  search the web  "}"#).unwrap();
        assert_eq!(request.need, "search the web");
    }

    #[test]
    fn parse_rejects_blank_need_and_unknown_fields() {
        assert!(CapabilityRequest::parse(r#"{"need":"  "}"#).is_err());
        assert!(CapabilityRequest::parse(r#"{"need":"x","extra":1}"#).is_err());
    }
}
