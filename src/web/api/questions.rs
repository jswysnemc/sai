use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use crate::question::{
    answer_question_with_images, cancel_question, pending_questions, QuestionAnswers,
};
use axum::extract::Path;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
struct QuestionAnswerRequest {
    #[serde(default)]
    answers: Option<QuestionAnswers>,
    #[serde(default)]
    cancelled: bool,
    #[serde(default)]
    image_urls: Vec<Vec<String>>,
}

/// 返回结构化提问路由。
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/questions/session/:id", get(list))
        .route("/api/questions/:id/answer", post(answer))
        .layer(axum::extract::DefaultBodyLimit::max(64 * 1024 * 1024))
}

/// 返回会话当前等待处理的提问。
async fn list(Path(id): Path<String>) -> Json<Vec<crate::question::PendingQuestion>> {
    Json(pending_questions(&id))
}

/// 提交结构化提问答案或取消。
async fn answer(
    Path(id): Path<String>,
    Json(request): Json<QuestionAnswerRequest>,
) -> WebResult<Json<Value>> {
    if request.cancelled {
        cancel_question(&id).map_err(WebError::from)?;
        return Ok(Json(json!({ "accepted": true, "status": "cancelled" })));
    }
    let Some(answers) = request.answers else {
        return Err(WebError::bad_request(
            "answers are required unless cancelled",
        ));
    };
    answer_question_with_images(&id, answers, request.image_urls).map_err(WebError::from)?;
    Ok(Json(json!({ "accepted": true, "status": "answered" })))
}
