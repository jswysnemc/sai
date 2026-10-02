use super::{ToolRegistry, ToolSpec};
use anyhow::bail;
use serde_json::json;

/// 注册 ask_question 占位工具。
///
/// 真正的交互由 Agent 循环通过 question broker 处理；
/// 直接调用该工具时返回错误，避免脱离会话使用。
pub fn register(registry: &mut ToolRegistry) {
    registry.register(ToolSpec::new(
        "ask_question",
        "在当前回复过程中向用户提出结构化问题，并等待回答后继续。仅在确实需要用户偏好、澄清或决策时使用。一次调用可包含多道问题；选项应互斥、简洁且带有有用说明。不要添加“其他”选项，单选与多选界面始终提供可编辑的“其他”答案。推荐项应放在第一项，并在标签中注明“（推荐）”。",
        json!({
            "type": "object",
            "properties": {
                "questions": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": 8,
                    "description": "要询问的问题列表。",
                    "items": {
                        "type": "object",
                        "properties": {
                            "header": {
                                "type": "string",
                                "description": "很短的问题标题，最多 30 个字符。"
                            },
                            "question": {
                                "type": "string",
                                "description": "完整、明确的问题。"
                            },
                            "options": {
                                "type": "array",
                                "maxItems": 8,
                                "description": "可选答案；无需添加“其他”。可以为空，此时仅允许用户输入自定义答案。",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "label": {
                                            "type": "string",
                                            "description": "简洁的选项文字。"
                                        },
                                        "description": {
                                            "type": "string",
                                            "description": "解释该选项的含义和影响。"
                                        }
                                    },
                                    "required": ["label", "description"],
                                    "additionalProperties": false
                                }
                            },
                            "multiple": {
                                "type": "boolean",
                                "description": "是否允许选择多个答案，默认 false。"
                            },
                            "custom": {
                                "type": "boolean",
                                "description": "兼容旧调用的参数；界面始终提供可编辑的 Other 自定义答案，传 false 也不会关闭。"
                            }
                        },
                        "required": ["header", "question", "options"],
                        "additionalProperties": false
                    }
                }
            },
            "required": ["questions"],
            "additionalProperties": false
        }),
        |_| async move { bail!("ask_question requires an active interactive Sai session") },
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【结构化提问】【入口兼容】旧参数先通过工具校验，再规范化为包含 Other 的请求；无参数和返回值。
    #[test]
    fn legacy_custom_false_reaches_question_normalization() {
        let mut registry = ToolRegistry::new();
        register(&mut registry);
        for multiple in [false, true] {
            let arguments = json!({"questions":[{
                "header":"范围", "question":"选择范围", "options":[], "multiple":multiple, "custom":false
            }]}).to_string();
            registry
                .validate_arguments("ask_question", &arguments)
                .unwrap();
            let request = crate::question::parse_ask_request(&arguments).unwrap();
            assert!(request.questions[0].custom);
        }
    }
}
