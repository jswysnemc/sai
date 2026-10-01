use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// 【Anthropic】【内容回传】按供应商索引合并原始块，保留思考签名和未知块字段。
#[derive(Default)]
pub(super) struct AnthropicContentAccumulator {
    blocks: BTreeMap<usize, Value>,
    arguments: BTreeMap<usize, String>,
}

impl AnthropicContentAccumulator {
    /// 【Anthropic】【内容回传】吸收一条 SSE 数据；参数为完整事件，返回结构校验结果。
    pub(super) fn observe(&mut self, event: &Value) -> Result<()> {
        let Some(index) = event
            .get("index")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
        else {
            return Ok(());
        };
        match event["type"].as_str() {
            Some("content_block_start") => {
                if let Some(block) = event.get("content_block") {
                    self.blocks.insert(index, block.clone());
                }
            }
            Some("content_block_delta") => {
                let delta = &event["delta"];
                if delta["type"] == "input_json_delta" {
                    if let Some(fragment) = delta["partial_json"].as_str() {
                        self.arguments.entry(index).or_default().push_str(fragment);
                    }
                    return Ok(());
                }
                let field = match delta["type"].as_str() {
                    Some("text_delta") => "text",
                    Some("thinking_delta") => "thinking",
                    Some("signature_delta") => "signature",
                    _ => return Ok(()),
                };
                if let (Some(block), Some(fragment)) =
                    (self.blocks.get_mut(&index), delta[field].as_str())
                {
                    match block.get_mut(field) {
                        Some(Value::String(text)) => text.push_str(fragment),
                        _ => block[field] = Value::String(fragment.to_owned()),
                    }
                }
            }
            Some("content_block_stop") => self.finish_arguments(index)?,
            _ => {}
        }
        Ok(())
    }

    /// 【Anthropic】【工具参数】解析完整参数并写回原始块；参数为块索引，返回 JSON 校验结果。
    fn finish_arguments(&mut self, index: usize) -> Result<()> {
        if let Some(arguments) = self.arguments.remove(&index) {
            if let Some(block) = self.blocks.get_mut(&index) {
                block["input"] = serde_json::from_str(&arguments)
                    .context("incomplete Anthropic tool arguments")?;
            }
        }
        Ok(())
    }

    /// 【Anthropic】【内容回传】完成剩余参数并返回有序内容块；无参数，返回原始块数组。
    pub(super) fn finish(mut self) -> Result<Vec<Value>> {
        for index in self.arguments.keys().copied().collect::<Vec<_>>() {
            self.finish_arguments(index)?;
        }
        Ok(self.blocks.into_values().collect())
    }
}

/// 【Anthropic】【提示缓存】未指定缓存策略时开启自动缓存，显式策略保持原样。
/// @param body 为已应用供应商自定义参数的请求体
/// @returns 无，原地补充缓存标记
pub(super) fn apply_default_cache_control(body: &mut Value) {
    let explicit = body.get("cache_control").is_some()
        || blocks_have_cache_control(&body["system"])
        || blocks_have_cache_control(&body["tools"])
        || body["messages"].as_array().is_some_and(|messages| {
            messages
                .iter()
                .any(|message| blocks_have_cache_control(&message["content"]))
        });
    if !explicit {
        body["cache_control"] = json!({"type": "ephemeral"});
    }
}

/// 【Anthropic】【提示缓存】只检查协议块属性，不把工具参数同名字段当成策略。
/// @param value 为内容块或工具数组；返回是否设置显式缓存断点
fn blocks_have_cache_control(value: &Value) -> bool {
    value.as_array().is_some_and(|blocks| {
        blocks
            .iter()
            .any(|block| block.get("cache_control").is_some())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【Anthropic】【原文回归】分片签名、删节思考和工具参数均保持顺序；无参数或返回值。
    #[test]
    fn retains_signed_and_redacted_blocks() {
        let mut accumulator = AnthropicContentAccumulator::default();
        let events = [
            json!({"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":"initial"}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":" thought"}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"sig"}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"nature"}}),
            json!({"type":"content_block_start","index":1,"content_block":{"type":"redacted_thinking","data":"opaque"}}),
            json!({"type":"content_block_start","index":2,"content_block":{"type":"tool_use","id":"call","name":"read_file","input":{}}}),
            json!({"type":"content_block_delta","index":2,"delta":{"type":"input_json_delta","partial_json":"{\"path\":"}}),
            json!({"type":"content_block_delta","index":2,"delta":{"type":"input_json_delta","partial_json":"\"a\"}"}}),
            json!({"type":"content_block_stop","index":2}),
        ];
        for event in events {
            accumulator.observe(&event).unwrap();
        }
        assert_eq!(
            accumulator.finish().unwrap(),
            vec![
                json!({"type":"thinking","thinking":"initial thought","signature":"signature"}),
                json!({"type":"redacted_thinking","data":"opaque"}),
                json!({"type":"tool_use","id":"call","name":"read_file","input":{"path":"a"}}),
            ]
        );
    }

    /// 【Anthropic】【缓存回归】补充默认策略，同时尊重顶层和块级覆盖；无参数或返回值。
    #[test]
    fn enables_caching_without_overwriting_explicit_policy() {
        let mut body = json!({"system":"stable"});
        apply_default_cache_control(&mut body);
        assert_eq!(body["cache_control"], json!({"type":"ephemeral"}));
        for explicit in [
            json!({"cache_control":null}),
            json!({"cache_control":{"type":"ephemeral","ttl":"1h"}}),
            json!({"system":[{"type":"text","text":"stable","cache_control":{"type":"ephemeral"}}]}),
        ] {
            let mut body = explicit.clone();
            apply_default_cache_control(&mut body);
            assert_eq!(body, explicit);
        }
    }
}
