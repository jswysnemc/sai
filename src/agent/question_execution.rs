use super::{tool_execution::RealToolExecution, Agent, AgentEvent};
use crate::question::{QuestionExchange, QuestionRequest, QuestionResponse};
use crate::tools::ToolModelAttachment;
use anyhow::Result;

impl Agent {
    /// 【结构化提问】【执行交互】解析问题、等待回答并生成文字结果和独立模型图片附件。
    /// @param arguments 工具参数；on_event 提问生命周期回调
    /// @returns 工具执行结果，图片不写入模型接收的 JSON 文本
    pub(super) async fn execute_question(
        &self,
        arguments: &str,
        on_event: &mut impl FnMut(AgentEvent) -> Result<()>,
    ) -> Result<RealToolExecution> {
        let request = match crate::question::parse_ask_request(arguments) {
            Ok(request) => request,
            Err(error) => {
                return Ok(failed_question(format!(
                    "invalid ask_question request: {error}"
                )))
            }
        };
        let (pending, receiver) =
            crate::question::request_question(self.session_id(), request.clone());
        let request_id = pending.id.clone();
        on_event(AgentEvent::QuestionRequested(pending))?;
        let response = receiver.await.unwrap_or(QuestionResponse::Cancelled);
        on_event(AgentEvent::QuestionResolved {
            request_id,
            response: response.clone(),
        })?;
        Ok(question_result(request, response))
    }
}

/// 【结构化提问】【结果转换】保持文字契约，把每题图片绑定到模型附件及可读计数。
/// @param request 原始问题；response 用户回答或取消状态
/// @returns 可持久化的工具结果及图片
fn question_result(request: QuestionRequest, response: QuestionResponse) -> RealToolExecution {
    let (answers, images) = match response {
        QuestionResponse::Answered(answers) => (answers, Vec::new()),
        QuestionResponse::AnsweredWithImages {
            answers,
            image_urls,
        } => (answers, image_urls),
        QuestionResponse::Cancelled => {
            return failed_question("user cancelled the question".into())
        }
        QuestionResponse::Unavailable(reason) => return failed_question(reason),
    };
    let exchange = match QuestionExchange::new(request, answers) {
        Ok(exchange) => exchange,
        Err(error) => return failed_question(format!("invalid ask_question answers: {error}")),
    };
    let mut output = crate::question::answered_tool_output(&exchange);
    let mut model_attachments = Vec::new();
    if let Ok(mut json) = serde_json::from_str::<serde_json::Value>(&output) {
        for (index, urls) in images.iter().enumerate() {
            if let Some(answer) = json["answers"].get_mut(index) {
                answer["image_count"] = serde_json::json!(urls.len());
            }
            for (image_index, url) in urls.iter().enumerate() {
                model_attachments.push(ToolModelAttachment::new(
                    url,
                    format!("question-{index}-image-{image_index}"),
                    format!(
                        "User answer image for question {}: {}",
                        index + 1,
                        exchange.questions[index].question
                    ),
                ));
            }
        }
        output = json.to_string();
    }
    RealToolExecution {
        output,
        model_attachments,
        failed: false,
    }
}

/// 【结构化提问】【错误结果】把取消或无效回答转换为失败输出。
/// @param reason 可读原因；返回不含图片的工具结果
fn failed_question(reason: String) -> RealToolExecution {
    RealToolExecution {
        output: crate::question::unavailable_tool_output(&reason),
        model_attachments: Vec::new(),
        failed: true,
    }
}
