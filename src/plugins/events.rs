use super::PluginSession;
use anyhow::Result;
use sai_plugin_runtime::{EventKind, InvocationContext};
use serde_json::Value;
use std::future::Future;

/// 【插件】【生命周期范围】只持有插件实例和可信上下文，不借用正在执行的 Agent。
#[derive(Clone)]
pub(crate) struct PluginEvents {
    session: PluginSession,
    context: InvocationContext,
}

impl PluginEvents {
    /// 【插件】【事件构造】为一次 Agent 操作固定实例与会话上下文。
    /// @param session 当前 Agent 的插件；context 为宿主可信资料
    /// @returns 可跨异步调用使用的事件分发器
    pub(crate) fn new(session: PluginSession, context: InvocationContext) -> Self {
        Self { session, context }
    }

    /// 【插件】【Agent 生命周期】在用户请求或子任务段前后各发一次事件。
    /// @param data 操作标识；operation 为真实业务 Future
    /// @returns 原操作结果；监听器失败不会替换操作结果
    pub(crate) fn agent_run<'a, T: 'a, F>(
        &'a self,
        data: Value,
        operation: F,
    ) -> impl Future<Output = Result<T>> + 'a
    where
        F: Future<Output = Result<T>> + 'a,
    {
        // 【插件】【栈空间】构造阶段即装箱，避免多层生命周期 Future 重复内嵌大型 Agent 状态机
        self.scope(
            EventKind::AgentStart,
            EventKind::AgentEnd,
            data,
            Box::pin(operation),
        )
    }

    /// 【插件】【模型轮次】一个逻辑模型请求对应一组轮次和消息事件，传输重试包含在同一范围内。
    /// @param data 为轮次标识；operation 为模型请求
    /// @returns 模型请求结果；工具执行有独立的 tool_call 和 tool_result 事件
    pub(crate) fn model_round<'a, T: 'a, F>(
        &'a self,
        data: Value,
        operation: F,
    ) -> impl Future<Output = Result<T>> + 'a
    where
        F: Future<Output = Result<T>> + 'a,
    {
        self.scope(
            EventKind::TurnStart,
            EventKind::TurnEnd,
            data.clone(),
            Box::pin(self.scope(
                EventKind::MessageStart,
                EventKind::MessageEnd,
                data,
                Box::pin(operation),
            )),
        )
    }

    /// 【插件】【事件配对】业务返回成功或失败后报告结束状态，不生成隐式模型消息。
    /// @param start、end 为事件类型；data 为操作标识；operation 为实际调用
    /// @returns 不经改写的业务结果；外部取消直接回收 Future，不后台补发回调
    async fn scope<T>(
        &self,
        start: EventKind,
        end: EventKind,
        data: Value,
        operation: impl Future<Output = Result<T>>,
    ) -> Result<T> {
        self.session
            .notify(start, &self.context, data.clone())
            .await;
        let result = operation.await;
        let mut data = data.as_object().cloned().unwrap_or_default();
        data.insert("ok".to_string(), Value::Bool(result.is_ok()));
        self.session
            .notify(end, &self.context, Value::Object(data))
            .await;
        result
    }
}
