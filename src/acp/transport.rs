use super::protocol::{self, Incoming, JsonRpcError, RequestId};
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{mpsc, oneshot, Mutex};

/// 对端主动发来的消息：请求需要回包，通知不需要。
#[derive(Debug)]
pub(crate) enum PeerMessage {
    Request {
        id: RequestId,
        method: String,
        params: Value,
    },
    Notification { method: String, params: Value },
}

/// 与 ACP agent 子进程的 stdio 连接。
///
/// 读取端独占一个任务：把响应按 id 派发给等待者，把对端主动发来的消息投递到队列。
/// 写入端加锁串行化，保证每条 JSON-RPC 消息占据完整一行。
/// 等待响应的请求表。
type PendingResponses = Arc<Mutex<HashMap<i64, oneshot::Sender<Result<Value, JsonRpcError>>>>>;

pub(crate) struct AcpTransport {
    /// 子进程句柄。字段本身不被读取，但必须持有：
    /// 它设了 kill_on_drop，丢弃即回收子进程，提前 drop 会让 agent 变成孤儿进程
    #[allow(dead_code)]
    child: Child,
    stdin: Arc<Mutex<ChildStdin>>,
    next_id: AtomicI64,
    pending: PendingResponses,
}

/// 对端消息接收端。
///
/// 与 `AcpTransport` 分开持有：一轮提示里 `session/prompt` 的响应与
/// 对端主动发来的请求要并发推进，合在一个对象上会构成可变与不可变借用冲突。
pub(crate) struct PeerReceiver {
    peer_rx: mpsc::UnboundedReceiver<PeerMessage>,
}

impl PeerReceiver {
    /// 取出对端发来的下一条消息。
    ///
    /// 返回:
    /// - 消息；连接结束时返回 None
    pub(crate) async fn recv(&mut self) -> Option<PeerMessage> {
        self.peer_rx.recv().await
    }
}

impl AcpTransport {
    /// 启动 ACP agent 子进程并接管其 stdio。
    ///
    /// 参数:
    /// - `program`: 启动程序
    /// - `args`: 启动参数
    /// - `env`: 追加的环境变量
    /// - `cwd`: 子进程工作目录
    ///
    /// 返回:
    /// - 已就绪的连接
    pub(crate) async fn spawn(
        program: &str,
        args: &[String],
        env: &std::collections::BTreeMap<String, String>,
        cwd: &std::path::Path,
    ) -> Result<(Self, PeerReceiver)> {
        let mut command = Command::new(program);
        command
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // stderr 交给父进程：适配器的下载进度与登录提示需要让用户看到
            .stderr(Stdio::inherit())
            .kill_on_drop(true);
        for (key, value) in env {
            command.env(key, value);
        }
        let mut child = command
            .spawn()
            .with_context(|| format!("failed to launch ACP agent: {program}"))?;
        let stdin = child
            .stdin
            .take()
            .context("ACP agent stdin is unavailable")?;
        let stdout = child
            .stdout
            .take()
            .context("ACP agent stdout is unavailable")?;
        let pending: PendingResponses = Arc::new(Mutex::new(HashMap::new()));
        let (peer_tx, peer_rx) = mpsc::unbounded_channel();
        let reader_pending = Arc::clone(&pending);
        // 读取任务随传输对象一起结束：Child 设了 kill_on_drop，管道关闭后循环自然退出
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let Some(incoming) = protocol::parse_incoming(&line) else {
                    continue;
                };
                match incoming {
                    Incoming::Response { id, result } => {
                        let RequestId::Number(id) = id else {
                            continue;
                        };
                        if let Some(sender) = reader_pending.lock().await.remove(&id) {
                            let _ = sender.send(result);
                        }
                    }
                    Incoming::Request { id, method, params } => {
                        let _ = peer_tx.send(PeerMessage::Request { id, method, params });
                    }
                    Incoming::Notification { method, params } => {
                        let _ = peer_tx.send(PeerMessage::Notification { method, params });
                    }
                }
            }
        });
        Ok((
            Self {
                child,
                stdin: Arc::new(Mutex::new(stdin)),
                next_id: AtomicI64::new(1),
                pending,
            },
            PeerReceiver { peer_rx },
        ))
    }

    /// 发送请求并等待响应。
    ///
    /// 参数:
    /// - `method`: 方法名
    /// - `params`: 参数
    ///
    /// 返回:
    /// - 对端返回的结果
    pub(crate) async fn request(&self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (sender, receiver) = oneshot::channel();
        self.pending.lock().await.insert(id, sender);
        if let Err(error) = self.write(&protocol::request(id, method, params)).await {
            self.pending.lock().await.remove(&id);
            return Err(error);
        }
        match receiver.await {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(error)) => bail!("ACP {method} failed: {error}"),
            // 发送端被丢弃说明读取任务已退出，通常是子进程崩溃
            Err(_) => bail!("ACP agent closed the connection during {method}"),
        }
    }

    /// 发送通知，不等待响应。
    ///
    /// 参数:
    /// - `method`: 方法名
    /// - `params`: 参数
    ///
    /// 返回:
    /// - 写入结果
    pub(crate) async fn notify(&self, method: &str, params: Value) -> Result<()> {
        self.write(&protocol::notification(method, params)).await
    }

    /// 回应对端请求。
    ///
    /// 参数:
    /// - `id`: 对端请求标识
    /// - `result`: 响应内容
    ///
    /// 返回:
    /// - 写入结果
    pub(crate) async fn respond(&self, id: &RequestId, result: Value) -> Result<()> {
        self.write(&protocol::response(id, result)).await
    }

    /// 以错误回应对端请求。
    ///
    /// 参数:
    /// - `id`: 对端请求标识
    /// - `code`: 错误码
    /// - `message`: 错误描述
    ///
    /// 返回:
    /// - 写入结果
    pub(crate) async fn respond_error(
        &self,
        id: &RequestId,
        code: i64,
        message: &str,
    ) -> Result<()> {
        self.write(&protocol::error_response(id, code, message))
            .await
    }

    /// 主动结束子进程。
    ///
    /// 丢弃传输对象时 kill_on_drop 已经会回收进程，这里提供的是显式的提前关闭，
    /// 供后续接入会话生命周期时使用。
    ///
    /// 返回:
    /// - 关闭结果
    #[allow(dead_code)]
    pub(crate) async fn shutdown(&mut self) -> Result<()> {
        let _ = self.child.start_kill();
        Ok(())
    }

    /// 写入一行 JSON 消息。
    ///
    /// 参数:
    /// - `value`: 待发送的 JSON 值
    ///
    /// 返回:
    /// - 写入结果
    async fn write(&self, value: &Value) -> Result<()> {
        let mut line = serde_json::to_string(value)?;
        line.push('\n');
        let mut stdin = self.stdin.lock().await;
        stdin.write_all(line.as_bytes()).await?;
        stdin.flush().await?;
        Ok(())
    }
}
