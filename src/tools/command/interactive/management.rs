use super::{session::InteractiveSession, sessions};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::sync::Arc;

/// 【交互命令】【面板归属】会话面板精确匹配 session，独立全局面板显示本进程所有交互会话。
/// 参数：会话对象及可选会话 ID；返回是否可管理。
fn in_scope(session: &InteractiveSession, owner: Option<&str>) -> bool {
    owner.is_none_or(|id| session.owner == format!("session:{id}"))
}

/// 【交互命令】【面板查找】校验会话归属后取得稳定引用；参数为任务 ID 和范围，返回会话。
fn lookup(id: &str, owner: Option<&str>) -> Result<Arc<InteractiveSession>> {
    sessions()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .get(id)
        .map(|(session, _)| session.clone())
        .filter(|session| in_scope(session, owner))
        .context("interactive command not found in this scope")
}

/// 【交互命令】【面板列表】列出运行和未消费完成会话，消费后注册表移除使其自动消失。
/// 参数：可选会话 ID；返回与后台面板兼容的任务记录。
pub(crate) fn list(owner: Option<&str>) -> Result<Vec<Value>> {
    let entries = sessions()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .iter()
        .filter(|(_, (session, _))| in_scope(session, owner))
        .map(|(id, (session, _))| (id.clone(), session.clone()))
        .collect::<Vec<_>>();
    entries.into_iter().map(|(id, session)| {
        let (code, _) = session.status()?;
        Ok(json!({"id":id,"label":"Interactive command","command":session.command,"cwd":session.cwd,
            "pid":session.pid,"status":if code.is_some() {"exited"} else {"running"},
            "started_at":session.started_at,"updated_at":session.started_at,"timeout_seconds":0}))
    }).collect()
}

/// 【交互命令】【面板输出】只读取未消费内容，不清空模型输入；参数为 ID 和范围，返回面板输出。
pub(crate) fn output(id: &str, owner: Option<&str>) -> Result<String> {
    let (stdout, truncated) = lookup(id, owner)?.peek_output();
    Ok(
        json!({"stdout":stdout,"stderr":"","stdout_truncated":truncated,"stderr_truncated":false})
            .to_string(),
    )
}

/// 【交互命令】【面板停止】终止所属会话的进程树；参数为 ID 和范围，返回结果，输出保留待消费。
pub(crate) fn stop(id: &str, owner: Option<&str>) -> Result<()> {
    lookup(id, owner)?.terminate();
    Ok(())
}

/// 【交互命令】【面板清理】显式清理只删除当前范围已退出会话；参数为范围，返回移除数量。
pub(crate) fn cleanup(owner: Option<&str>) -> usize {
    let mut registry = sessions().lock().unwrap_or_else(|error| error.into_inner());
    let before = registry.len();
    registry.retain(|_, (session, _)| {
        !in_scope(session, owner)
            || !session
                .status()
                .is_ok_and(|(code, eof)| code.is_some() && eof)
    });
    before - registry.len()
}
