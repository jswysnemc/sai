use super::*;
use std::future::{poll_fn, Future};
use std::task::Poll;
use std::time::Duration;

/// 【IPC】【Windows 回归】连接已到达但等待被心跳取消时，后续 accept 仍能交接同一管道。
/// 参数：无；返回：连接建立或帧往返失败时返回错误。
#[tokio::test]
async fn cancelling_accept_preserves_the_connected_pipe() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let holder = WinPipeTransport::bind(directory.path())?;
    let observer = WinPipeTransport::bind(directory.path())?;
    let mut accepting = Box::pin(holder.accept());

    // 1. 【IPC】【Windows 回归】明确推进到等待连接的位置，不依赖睡眠或线程调度速度
    poll_fn(|context| {
        assert!(accepting.as_mut().poll(context).is_pending());
        Poll::Ready(())
    })
    .await;
    let mut client = observer.connect().await?;
    // 2. 【IPC】【Windows 回归】模拟 select! 选择心跳分支，丢弃上一轮连接等待
    drop(accepting);
    let mut server = tokio::time::timeout(Duration::from_secs(2), holder.accept())
        .await
        .context("取消等待后，已到达的连接未被保留")??;

    // 3. 【IPC】【Windows 回归】用真实命名管道确认原连接仍能双向收发
    let hello = Frame::control(
        crate::ipc::frame::KIND_CTL_HELLO,
        serde_json::json!({"after": 2}),
    );
    client.send(&hello).await?;
    assert_eq!(server.recv().await?, Some(hello));
    let replay = Frame {
        kind: crate::ipc::frame::KIND_EVT_MIRROR.to_string(),
        sequence: Some(3),
        payload: serde_json::json!({"text": "replayed"}),
    };
    server.send(&replay).await?;
    assert_eq!(client.recv().await?, Some(replay));
    Ok(())
}
