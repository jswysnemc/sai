use super::{QuestionAnswers, QuestionRequest, QuestionResponse, validate_answers};
use anyhow::{bail, Result};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use tokio::sync::oneshot;
use uuid::Uuid;

/// 等待用户回答的结构化提问请求。
#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub(crate) struct PendingQuestion {
    pub(crate) id: String,
    pub(crate) session_id: String,
    pub(crate) request: QuestionRequest,
}

struct PendingEntry {
    question: PendingQuestion,
    sender: oneshot::Sender<QuestionResponse>,
}

/// 返回进程内共享的等待提问表。
fn pending() -> &'static Mutex<HashMap<String, PendingEntry>> {
    static PENDING: OnceLock<Mutex<HashMap<String, PendingEntry>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 创建提问请求并等待用户回答。
pub(crate) fn request_question(
    session_id: &str,
    request: QuestionRequest,
) -> (PendingQuestion, oneshot::Receiver<QuestionResponse>) {
    let question = PendingQuestion {
        id: Uuid::new_v4().to_string(),
        session_id: session_id.to_string(),
        request,
    };
    let (sender, receiver) = oneshot::channel();
    pending().lock().unwrap().insert(
        question.id.clone(),
        PendingEntry {
            question: question.clone(),
            sender,
        },
    );
    (question, receiver)
}

/// 提交结构化回答并唤醒等待中的工具。
pub(crate) fn answer_question(id: &str, answers: QuestionAnswers) -> Result<()> {
    let mut map = pending().lock().unwrap();
    let Some(entry) = map.get(id) else {
        bail!("question request is no longer pending")
    };
    // 先校验，失败时保留 pending，避免用户点错后请求丢失导致整轮挂死
    validate_answers(&entry.question.request, &answers)?;
    let entry = map
        .remove(id)
        .expect("pending question must still exist after validation");
    entry
        .sender
        .send(QuestionResponse::Answered(answers))
        .map_err(|_| anyhow::anyhow!("question requester is no longer running"))
}

/// 取消当前提问。
pub(crate) fn cancel_question(id: &str) -> Result<()> {
    let Some(entry) = pending().lock().unwrap().remove(id) else {
        bail!("question request is no longer pending")
    };
    entry
        .sender
        .send(QuestionResponse::Cancelled)
        .map_err(|_| anyhow::anyhow!("question requester is no longer running"))
}

/// 将提问标记为不可用（例如交互界面无法打开）。
pub(crate) fn unavailable_question(id: &str, reason: impl Into<String>) -> Result<()> {
    let Some(entry) = pending().lock().unwrap().remove(id) else {
        bail!("question request is no longer pending")
    };
    entry
        .sender
        .send(QuestionResponse::Unavailable(reason.into()))
        .map_err(|_| anyhow::anyhow!("question requester is no longer running"))
}

/// 提交完整 QuestionResponse。
pub(crate) fn resolve_question(id: &str, response: QuestionResponse) -> Result<()> {
    match response {
        QuestionResponse::Answered(answers) => answer_question(id, answers),
        QuestionResponse::Cancelled => cancel_question(id),
        QuestionResponse::Unavailable(reason) => unavailable_question(id, reason),
    }
}

/// 返回指定会话当前等待处理的提问。
pub(crate) fn pending_questions(session_id: &str) -> Vec<PendingQuestion> {
    pending()
        .lock()
        .unwrap()
        .values()
        .filter(|entry| entry.question.session_id == session_id)
        .map(|entry| entry.question.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::question::{QuestionOption, QuestionPrompt};

    fn sample_request() -> QuestionRequest {
        QuestionRequest {
            questions: vec![QuestionPrompt {
                header: "范围".to_string(),
                question: "修改哪些文件？".to_string(),
                options: vec![QuestionOption {
                    label: "全部".to_string(),
                    description: "全部相关文件".to_string(),
                }],
                multiple: false,
                custom: true,
            }],
        }
    }

    #[tokio::test]
    async fn question_request_waits_for_answer() {
        let (pending, receiver) = request_question("session", sample_request());
        assert!(pending_questions("session")
            .iter()
            .any(|item| item.id == pending.id));
        answer_question(&pending.id, vec![vec!["全部".to_string()]]).unwrap();
        assert!(matches!(
            receiver.await.unwrap(),
            QuestionResponse::Answered(_)
        ));
    }
}
