use super::QuestionRequest;
use anyhow::Result;

/// 【结构化提问】【请求规范化】参数为 ask 工具 JSON，返回始终允许 Other 的已校验请求。
/// 旧调用的 custom=false 仍可解析；第三方表单的通用问题模型不受影响。
pub(crate) fn parse_ask_request(arguments: &str) -> Result<QuestionRequest> {
    let mut request: QuestionRequest = serde_json::from_str(arguments)?;
    for question in &mut request.questions {
        question.custom = true;
    }
    request.validate()?;
    Ok(request)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::question::validate_answers;

    /// 【结构化提问】【Other 回归】两种题型均允许自定义回答，包括旧调用明确关闭的情况；无参数和返回值。
    #[test]
    fn ask_always_accepts_other_for_single_and_multiple() {
        for multiple in [false, true] {
            let request = parse_ask_request(&serde_json::json!({"questions":[{
                "header":"范围", "question":"修改范围？", "options":[], "multiple":multiple, "custom":false
            }]}).to_string()).unwrap();
            assert!(request.questions[0].custom);
            assert!(validate_answers(&request, &vec![vec!["用户补充".into()]]).is_ok());
        }
    }
}
