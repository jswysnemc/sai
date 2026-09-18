use crate::{
    runner::{ActorHandle, SessionSubscription},
    web::runs::WebEvent,
};
use axum::{extract::Request, middleware::Next, response::Response};
use std::{io::Write, time::Instant};

/// 随会话销毁的控制台订阅，避免后台任务继续持有已删除会话。
pub(crate) struct ConsoleSubscription(tokio::task::JoinHandle<()>);

impl Drop for ConsoleSubscription {
    /// 【Web】【释放日志】取消订阅任务并释放其事件总线；无参数，无返回值
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// 【Web】【运行日志】即时输出一行带时间和级别的服务日志。
/// 参数: action 为动作，message 为不含密钥或正文的摘要，error 为错误级别标志
/// 返回: 无；终端关闭时忽略输出错误
pub(super) fn write(action: &str, message: &str, error: bool) {
    let mut output = std::io::stderr().lock();
    let level = if error { "ERROR" } else { "INFO" };
    let _ = writeln!(
        output,
        "{} {level} 【Web】【{action}】{message}",
        chrono::Local::now().format("%H:%M:%S")
    );
    let _ = output.flush();
}

/// 【Web】【请求日志】记录请求开始及响应头就绪，流式连接无需等待正文结束。
/// 参数: request 为请求，next 为后续路由
/// 返回: 原始响应
pub(super) async fn request(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    // 1. 【Web】【日志隐私】只使用路径，不记录含启动令牌的查询串、头部或请求正文
    let path = safe_field(request.uri().path());
    let started = Instant::now();
    write("请求开始", &format!("{method} {path}"), false);
    let response = next.run(request).await;
    write(
        "响应就绪",
        &format!(
            "{method} {path} status={} elapsed_ms={}",
            response.status().as_u16(),
            started.elapsed().as_millis()
        ),
        response.status().is_server_error(),
    );
    response
}

/// 【Web】【事件日志】订阅会话的新事件，不重复打印磁盘历史；积压后按序号补发并重连。
/// 参数: bus 为当前会话事件总线
/// 返回: 随会话保存的订阅守卫；释放守卫时停止日志任务
pub(super) fn subscribe_runs(bus: &ActorHandle) -> Option<ConsoleSubscription> {
    let subscription = bus.attach()?;
    let bus = bus.clone();
    Some(ConsoleSubscription(tokio::spawn(run_subscription(
        bus,
        subscription,
        |message, error| {
            write("运行事件", &message, error);
        },
    ))))
}

/// 【Web】【事件订阅】消费摘要事件；订阅积压后从日志补发并重新连接。
/// 参数: bus 为事件总线，subscription 为当前订阅，sink 为摘要输出函数
/// 返回: 事件总线关闭或无法重新订阅后的完成通知
async fn run_subscription<F>(bus: ActorHandle, mut subscription: SessionSubscription, mut sink: F)
where
    F: FnMut(String, bool) + Send + 'static,
{
    let mut last_sequence = 0;
    loop {
        while let Some(event) = subscription.events.recv().await {
            if event.sequence <= last_sequence {
                continue;
            }
            last_sequence = event.sequence;
            if let Some(message) = run_summary(&event) {
                sink(message, event.kind == "run.failed");
            }
        }
        if subscription.dropped_events() == 0 {
            break;
        }
        sink(
            "控制台订阅落后，正在从会话日志补发并重新连接".to_string(),
            true,
        );
        // 1. 先订阅再读取历史，订阅入队前发生的事件由历史补齐，重叠事件按序号去重
        let Some(next) = bus.attach_ready().await else {
            break;
        };
        subscription = next;
        // 2. 从落盘日志补发断点后的事件
        for event in bus.replay(last_sequence) {
            if event.sequence <= last_sequence {
                continue;
            }
            last_sequence = event.sequence;
            if let Some(message) = run_summary(&event) {
                sink(message, event.kind == "run.failed");
            }
        }
    }
}

/// 【Web】【日志字段】去除控制字符并限制字段长度，避免终端注入。
/// 参数: value 为不可信的日志字段；返回单行短文本
fn safe_field(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !ch.is_control())
        .take(200)
        .collect()
}

/// 【Web】【事件摘要】仅记录白名单事件的标识，模型正文和工具参数不输出。
/// 参数: event 为运行事件；返回可选日志摘要
fn run_summary(event: &WebEvent) -> Option<String> {
    if !matches!(
        event.kind.as_str(),
        "run.queued"
            | "run.started"
            | "run.completed"
            | "run.failed"
            | "run.interrupted"
            | "tool.call.started"
            | "tool.result"
            | "status.changed"
            | "permission.requested"
            | "compaction.started"
            | "compaction.finished"
    ) {
        return None;
    }
    Some(format!(
        "session={} run={} event={}{}",
        safe_field(&event.session_id),
        safe_field(&event.run_id),
        event.kind,
        event
            .payload
            .get("status")
            .and_then(serde_json::Value::as_str)
            .map(|status| format!(" status={}", safe_field(status)))
            .unwrap_or_default()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::SessionActor;
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    /// 【Web】【日志回收】释放守卫后总线和订阅均关闭；无参数，无返回值
    #[tokio::test]
    async fn dropping_console_subscription_releases_event_bus() {
        let temp = tempfile::tempdir().unwrap();
        let bus = SessionActor::spawn(temp.path().join("events.jsonl"), "workspace", "session");
        let mut observer = bus.attach_ready().await.unwrap();
        let guard = subscribe_runs(&bus).unwrap();
        drop(bus);
        drop(guard);
        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(1), observer.events.recv())
                .await
                .unwrap()
                .is_none()
        );
    }
    /// 【Web】【日志验证】请求正文和工具参数不能泄漏到事件日志；无参数，无返回值
    #[test]
    fn summary_excludes_private_payloads_and_streaming_content() {
        let mut event = WebEvent::new(
            "run",
            "workspace",
            "session\n",
            "tool.call.started",
            serde_json::json!({"arguments":"secret", "input":"private"}),
        );
        let summary = run_summary(&event).unwrap();
        assert!(summary.contains("event=tool.call.started"));
        assert!(
            !summary.contains("secret") && !summary.contains("private") && !summary.contains('\n')
        );
        event.kind = "message.content.delta".into();
        assert!(run_summary(&event).is_none());
    }

    /// 【Web】【日志恢复】订阅积压后能补发摘要并继续接收新事件；异步，无返回值
    #[tokio::test]
    async fn subscription_reconnects_after_backlog() {
        let temp = tempfile::tempdir().unwrap();
        let bus = SessionActor::spawn(temp.path().join("events.jsonl"), "workspace", "session");
        let subscription = bus.attach().unwrap();
        for index in 0..2048 {
            bus.emit(WebEvent::new(
                "run",
                "workspace",
                "session",
                "message.content.delta",
                json!({"text": index}),
            ))
            .unwrap();
        }
        bus.emit(WebEvent::new(
            "run",
            "workspace",
            "session",
            "run.completed",
            json!({}),
        ))
        .unwrap();
        for _ in 0..200 {
            if bus.journal().events_after(0).len() >= 2048 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        let summaries = Arc::new(Mutex::new(Vec::<String>::new()));
        let received = summaries.clone();
        let logger = tokio::spawn(run_subscription(
            bus.clone(),
            subscription,
            move |message, _| {
                received.lock().unwrap().push(message);
            },
        ));
        for _ in 0..200 {
            if summaries
                .lock()
                .unwrap()
                .iter()
                .any(|message| message.contains("run.completed"))
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        bus.emit(WebEvent::new(
            "run",
            "workspace",
            "session",
            "run.completed",
            json!({"second": true}),
        ))
        .unwrap();
        for _ in 0..200 {
            if summaries.lock().unwrap().len() >= 3 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        assert!(
            summaries
                .lock()
                .unwrap()
                .iter()
                .filter(|message| message.contains("run.completed"))
                .count()
                >= 2
        );
        logger.abort();
        let _ = logger.await;
    }
}
