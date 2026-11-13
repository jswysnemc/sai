//! 模型尝试与取消配对，重试层每次实际请求分别报告，不混同整个用户轮次。
use super::{HookContext, HookEvent};
use crate::{config::HooksConfig, tools::ToolRegistry};
use std::future::Future;
tokio::task_local! { static MODEL_SCOPE: (HooksConfig,HookContext,ToolRegistry); }
/// 【Hooks】【模型作用域】参数为配置、上下文、注册表和请求 future，返回原请求结果
pub(crate) async fn model_scope<T>(
    config: HooksConfig,
    context: HookContext,
    tools: ToolRegistry,
    work: impl Future<Output = T>,
) -> T {
    MODEL_SCOPE.scope((config, context, tools), work).await
}
pub(crate) struct ModelAttempt {
    scope: Option<(HooksConfig, HookContext, ToolRegistry)>,
}
impl ModelAttempt {
    /// 【Hooks】【请求开始】参数为尝试序号，返回取消保护器
    pub(crate) async fn begin(attempt: usize) -> Self {
        let scope = MODEL_SCOPE
            .try_with(Clone::clone)
            .ok()
            .map(|(config, context, tools)| (config, context.with("attempt", attempt), tools));
        if let Some((config, context, tools)) = &scope {
            super::dispatch_with_tools(config, HookEvent::PreModelRequest, context, Some(tools))
                .await;
        }
        Self { scope }
    }
    /// 【Hooks】【请求结束】参数为模型结果，发送完成或失败事件，返回无值
    pub(crate) async fn finish(&mut self, result: &anyhow::Result<crate::llm::ChatResult>) {
        if let Some((config, context, tools)) = self.scope.take() {
            let context = context
                .with(
                    "status",
                    if result.is_ok() {
                        "completed"
                    } else {
                        "failed"
                    },
                )
                .with(
                    "error",
                    result.as_ref().err().map(|error| error.to_string()),
                )
                .with(
                    "token_usage",
                    result
                        .as_ref()
                        .ok()
                        .and_then(|result| result.usage.as_ref()),
                );
            super::dispatch_with_tools(
                &config,
                HookEvent::PostModelRequest,
                &context,
                Some(&tools),
            )
            .await;
        }
    }
}
impl Drop for ModelAttempt {
    /// 【Hooks】【请求取消】无参数，在活动请求 future 被取消后报告 cancelled
    fn drop(&mut self) {
        if let Some((config, context, tools)) = self.scope.take() {
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                runtime.spawn(async move {
                    super::dispatch_with_tools(
                        &config,
                        HookEvent::PostModelRequest,
                        &context.with("status", "cancelled"),
                        Some(&tools),
                    )
                    .await;
                });
            }
        }
    }
}
