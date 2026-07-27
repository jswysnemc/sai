use agent_client_protocol::schema::v1::{
    CreateElicitationRequest, CreateElicitationResponse, ElicitationAcceptAction,
    ElicitationAction, ElicitationContentValue,
};
use anyhow::Result;
use serde_json::Value;
use std::collections::BTreeMap;

/// 处理 ACP agent 发起的结构化信息征询。
///
/// 参数:
/// - `params`: `elicitation/create` 参数
/// - `events`: 统一 Agent 事件发送端
/// - `session_id`: Sai 宿主会话标识
///
/// 返回:
/// - 标准 ACP elicitation response JSON
pub(crate) async fn handle(
    params: &Value,
    events: &crate::agent_engine::EventSender,
    session_id: &str,
) -> Result<Value> {
    let request: CreateElicitationRequest =
        super::sdk::from_value(params.clone(), "elicitation/create request")?;
    let (question, fields) = question_from_schema(params, &request.message);
    let (pending, receiver) = crate::question::request_question(session_id, question);
    let request_id = pending.id.clone();
    let _ = events.send(crate::agent::AgentEvent::QuestionRequested(pending));
    let response = receiver
        .await
        .unwrap_or(crate::question::QuestionResponse::Cancelled);
    let _ = events.send(crate::agent::AgentEvent::QuestionResolved {
        request_id,
        response: response.clone(),
    });
    let action = match response {
        crate::question::QuestionResponse::Answered(answers) => {
            let content = fields
                .into_iter()
                .zip(answers)
                .filter_map(|(field, answers)| {
                    answer_value(&field.kind, answers).map(|value| (field.name, value))
                })
                .collect::<BTreeMap<_, _>>();
            ElicitationAction::Accept(ElicitationAcceptAction::new().content(content))
        }
        crate::question::QuestionResponse::Cancelled
        | crate::question::QuestionResponse::Unavailable(_) => ElicitationAction::Cancel,
    };
    super::sdk::to_value(&CreateElicitationResponse::new(action))
}

/// ACP 表单字段的回答转换信息。
struct ElicitationField {
    name: String,
    kind: String,
}

/// 将 ACP JSON Schema 转成 Sai 的统一问题模型。
///
/// 参数:
/// - `params`: 原始 elicitation 参数
/// - `message`: agent 提供的说明
///
/// 返回:
/// - 问题请求与回答字段顺序
fn question_from_schema(
    params: &Value,
    message: &str,
) -> (crate::question::QuestionRequest, Vec<ElicitationField>) {
    let requested_schema = params.get("requestedSchema");
    let properties = requested_schema
        .and_then(|schema| schema.get("properties"))
        .and_then(Value::as_object);
    let required = requested_schema
        .and_then(|schema| schema.get("required"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let mut fields = Vec::new();
    let mut questions = Vec::new();
    if let Some(properties) = properties {
        for (name, schema) in properties {
            let kind = schema
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("string")
                .to_string();
            let options = schema_options(schema, &kind);
            let default_answers = schema
                .get("default")
                .map(json_values_as_answers)
                .unwrap_or_default();
            questions.push(crate::question::QuestionPrompt {
                header: compact_header(name),
                question: schema
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or(message)
                    .to_string(),
                custom: options.is_empty(),
                options,
                multiple: kind == "array",
                required: required.contains(name.as_str()),
                default_answers,
                validation: Some(question_validation(schema, &kind)),
            });
            fields.push(ElicitationField {
                name: name.clone(),
                kind,
            });
        }
    }
    if questions.is_empty() {
        questions.push(crate::question::QuestionPrompt {
            header: "Response".to_string(),
            question: message.to_string(),
            options: Vec::new(),
            multiple: false,
            custom: true,
            required: true,
            default_answers: Vec::new(),
            validation: None,
        });
        fields.push(ElicitationField {
            name: "value".to_string(),
            kind: "string".to_string(),
        });
    }
    (crate::question::QuestionRequest { questions }, fields)
}

/// 将 schema 的 enum、oneOf 或布尔类型转换为显示名称与真实值分离的选项。
///
/// 参数:
/// - `schema`: 单个属性 schema
/// - `kind`: 属性类型
///
/// 返回:
/// - 可供统一问题界面展示的选项
fn schema_options(schema: &Value, kind: &str) -> Vec<crate::question::QuestionOption> {
    if let Some(variants) = schema.get("oneOf").and_then(Value::as_array) {
        let options = variants
            .iter()
            .filter_map(|variant| {
                let value = scalar_as_answer(variant.get("const")?)?;
                let label = variant
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or(&value)
                    .to_string();
                Some(crate::question::QuestionOption {
                    label,
                    description: variant
                        .get("description")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    value: Some(value),
                })
            })
            .collect::<Vec<_>>();
        if !options.is_empty() {
            return options;
        }
    }
    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        return values
            .iter()
            .filter_map(scalar_as_answer)
            .map(|value| crate::question::QuestionOption {
                label: value.clone(),
                description: String::new(),
                value: Some(value),
            })
            .collect();
    }
    if kind == "boolean" {
        return ["true", "false"]
            .into_iter()
            .map(|value| crate::question::QuestionOption {
                label: value.to_string(),
                description: String::new(),
                value: Some(value.to_string()),
            })
            .collect();
    }
    Vec::new()
}

/// 从 JSON Schema 提取通用答案约束。
///
/// 参数:
/// - `schema`: 单个属性 schema
/// - `kind`: 属性类型
///
/// 返回:
/// - 通用问题校验配置
fn question_validation(schema: &Value, kind: &str) -> crate::question::QuestionValidation {
    crate::question::QuestionValidation {
        value_type: Some(kind.to_string()),
        minimum: schema.get("minimum").and_then(Value::as_number).cloned(),
        maximum: schema.get("maximum").and_then(Value::as_number).cloned(),
        min_length: schema
            .get("minLength")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok()),
        max_length: schema
            .get("maxLength")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok()),
        pattern: schema
            .get("pattern")
            .and_then(Value::as_str)
            .map(str::to_string),
        format: schema
            .get("format")
            .and_then(Value::as_str)
            .map(str::to_string),
    }
}

/// 将 JSON 默认值转换为统一问题答案数组。
///
/// 参数:
/// - `value`: JSON Schema default 值
///
/// 返回:
/// - 可直接作为初始回答的字符串数组
fn json_values_as_answers(value: &Value) -> Vec<String> {
    match value {
        Value::Array(values) => values.iter().filter_map(scalar_as_answer).collect(),
        value => scalar_as_answer(value).into_iter().collect(),
    }
}

/// 将 JSON 标量转换为表单传输字符串。
///
/// 参数:
/// - `value`: JSON 标量
///
/// 返回:
/// - 字符串、数值或布尔值的文本表示
fn scalar_as_answer(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

/// 把表单字段名限制到 Sai 问题标题允许的长度。
///
/// 参数:
/// - `name`: ACP schema 属性名
///
/// 返回:
/// - 不超过 30 个字符的标题
fn compact_header(name: &str) -> String {
    name.chars().take(30).collect()
}

/// 按 JSON Schema 基础类型转换用户回答。
///
/// 参数:
/// - `kind`: schema `type`
/// - `answers`: 用户对该字段选择或填写的值
///
/// 返回:
/// - ACP elicitation 内容值
fn answer_value(kind: &str, answers: Vec<String>) -> Option<ElicitationContentValue> {
    if kind == "array" {
        return Some(ElicitationContentValue::StringArray(answers));
    }
    let answer = answers.into_iter().next()?;
    match kind {
        "integer" => answer
            .parse::<i64>()
            .ok()
            .map(ElicitationContentValue::Integer),
        "number" => answer
            .parse::<f64>()
            .ok()
            .map(ElicitationContentValue::Number),
        "boolean" => answer
            .parse::<bool>()
            .ok()
            .map(ElicitationContentValue::Boolean),
        _ => Some(ElicitationContentValue::String(answer)),
    }
}

#[cfg(test)]
mod tests {
    use super::question_from_schema;

    /// 表单属性必须按 schema 顺序转换为统一问题。
    #[test]
    fn converts_form_schema_to_questions() {
        let params = serde_json::json!({
            "requestedSchema": {
                "properties": {
                    "branch": { "type": "string", "enum": ["main", "dev"] }
                }
            }
        });
        let (request, fields) = question_from_schema(&params, "Choose branch");
        assert_eq!(request.questions.len(), 1);
        assert_eq!(request.questions[0].options.len(), 2);
        assert_eq!(fields[0].name, "branch");
    }

    /// oneOf 的标题只用于展示，提交时必须保留 const 真实值。
    #[test]
    fn preserves_one_of_values_and_schema_constraints() {
        let params = serde_json::json!({
            "requestedSchema": {
                "required": ["effort"],
                "properties": {
                    "effort": {
                        "type": "integer",
                        "oneOf": [
                            { "const": 1, "title": "Low" },
                            { "const": 3, "title": "High" }
                        ],
                        "default": 3,
                        "minimum": 1,
                        "maximum": 3
                    }
                }
            }
        });
        let (request, _) = question_from_schema(&params, "Choose effort");
        let question = &request.questions[0];

        assert!(question.required);
        assert_eq!(question.default_answers, vec!["3"]);
        assert_eq!(question.options[0].label, "Low");
        assert_eq!(question.options[0].answer_value(), "1");
        assert_eq!(
            question
                .validation
                .as_ref()
                .unwrap()
                .minimum
                .as_ref()
                .and_then(serde_json::Number::as_i64),
            Some(1)
        );
    }

    /// 未列入 required 的属性必须允许空回答。
    #[test]
    fn marks_optional_fields_as_skippable() {
        let params = serde_json::json!({
            "requestedSchema": {
                "properties": {
                    "note": { "type": "string", "minLength": 2 }
                }
            }
        });
        let (request, _) = question_from_schema(&params, "Optional note");

        assert!(!request.questions[0].required);
        assert!(crate::question::validate_answers(&request, &vec![Vec::new()]).is_ok());
        assert!(crate::question::validate_answers(&request, &vec![vec!["x".to_string()]]).is_err());
    }
}
