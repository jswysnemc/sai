use crate::runner::{RunnerSubmission, RunnerSubmissionKind};

/// 【会话同步】【轮次标识】统一终端广播和持久化使用的运行标识。
/// 参数: submission 为待执行提交
/// 返回: 运行标识、用户正文与图片列表
pub(super) fn prepare_run_identity(
    submission: &mut RunnerSubmission,
) -> (String, String, Vec<String>) {
    match &mut submission.kind {
        RunnerSubmissionKind::UserInput(input) => {
            let run_id = input
                .turn_id
                .get_or_insert_with(|| format!("run_{}", uuid::Uuid::new_v4().simple()))
                .clone();
            (run_id, input.input.clone(), input.image_urls.clone())
        }
        _ => (
            format!("run_{}", uuid::Uuid::new_v4().simple()),
            String::new(),
            Vec::new(),
        ),
    }
}

#[cfg(test)]
#[path = "identity_tests.rs"]
mod tests;
