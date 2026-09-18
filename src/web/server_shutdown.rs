use super::{runs::RunManager, server_logging, terminal::TerminalManager};
use anyhow::{Context, Result};
use axum::Router;
use std::{
    future::{Future, IntoFuture},
    time::Duration,
};

/// 【Web】【退出信号】等待 Ctrl+C，Unix 同时处理 SIGTERM。
/// 参数: 无；返回信号监听结果
pub(super) async fn signal() -> Result<()> {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => result.context("failed to listen for Ctrl+C"),
            _ = terminate.recv() => Ok(()),
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c()
            .await
            .context("failed to listen for Ctrl+C")
    }
}

/// 【Web】【服务生命周期】收到信号后关闭监听并限时回收资源，长连接不能无限阻塞退出。
/// 参数: listener 为监听器，app 为路由，stop 为退出信号，cleanup 为清理任务，grace 为最长等待时间
/// 返回: HTTP 服务或信号处理结果
pub(super) async fn serve_until<S, C>(
    listener: tokio::net::TcpListener,
    app: Router,
    stop: S,
    cleanup: C,
    grace: Duration,
) -> Result<()>
where
    S: Future<Output = Result<()>>,
    C: Future<Output = ()>,
{
    let (send, receive) = tokio::sync::oneshot::channel();
    let server = axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = receive.await;
        })
        .into_future();
    tokio::pin!(server);
    tokio::select! {
        result = &mut server => return result.context("Web server failed"),
        result = stop => result?,
    }
    // 1. 【Web】【关闭入口】先停止接受连接，再回收任务，队列保留供下次启动恢复
    server_logging::write("关闭服务", "正在停止活动运行和终端会话", false);
    let _ = send.send(());
    let drain = async {
        let (_, result) = tokio::join!(cleanup, &mut server);
        result
    };
    match tokio::time::timeout(grace, drain).await {
        Ok(result) => result.context("Web server shutdown failed")?,
        Err(_) => server_logging::write("关闭服务", "等待上限已到，关闭剩余长连接", false),
    }
    server_logging::write("服务停止", "Sai Web 已退出", false);
    Ok(())
}

/// 【Web】【资源回收】同时终止本进程运行与浏览器终端。
/// 参数: runs 为运行管理器，terminals 为终端管理器；返回无
pub(super) async fn cleanup(runs: RunManager, terminals: TerminalManager) {
    let (runs, terminals) = tokio::join!(runs.shutdown(), terminals.shutdown());
    for result in [runs, terminals] {
        if let Err(error) = result {
            server_logging::write("资源清理失败", &format!("{error:#}"), true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{response::Sse, routing::get};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    /// 【Web】【退出验证】保持 SSE 连接时仍在期限内退出且执行清理；无参数，无返回值
    #[tokio::test]
    async fn open_sse_connection_cannot_block_shutdown() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = Router::new().route(
            "/events",
            get(|| async {
                Sse::new(futures_util::stream::pending::<
                    Result<axum::response::sse::Event, std::convert::Infallible>,
                >())
            }),
        );
        let (send, receive) = tokio::sync::oneshot::channel();
        let cleaned = Arc::new(AtomicBool::new(false));
        let flag = cleaned.clone();
        let server = tokio::spawn(serve_until(
            listener,
            app,
            async {
                receive.await?;
                Ok(())
            },
            async move {
                flag.store(true, Ordering::SeqCst);
            },
            Duration::from_millis(50),
        ));
        let response = reqwest::get(format!("http://{address}/events"))
            .await
            .unwrap();
        assert!(response.status().is_success());
        send.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(1), server)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(cleaned.load(Ordering::SeqCst));
        drop(response);
    }
}
