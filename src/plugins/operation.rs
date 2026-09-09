use std::future::Future;

tokio::task_local! { static OPERATION: String; }

/// 【插件操作】【可信标识】同一用户操作中的普通工具与嵌套插件共享标识。
/// @returns 当前标识，独立直接调用会生成新标识
pub(crate) fn current() -> String {
    OPERATION
        .try_with(Clone::clone)
        .unwrap_or_else(|_| uuid::Uuid::new_v4().to_string())
}

/// 【插件操作】【作用域】嵌套生命周期沿用外层标识，避免伪造新一轮确认。
/// @param id 进入调用前生成的标识；operation 为已装箱的业务 Future
/// @returns 带可信操作标识的 Future，不改变原操作结果
pub(crate) fn scope<F: Future>(id: &str, operation: F) -> impl Future<Output = F::Output> {
    let id = OPERATION
        .try_with(Clone::clone)
        .unwrap_or_else(|_| id.to_string());
    OPERATION.scope(id, operation)
}
