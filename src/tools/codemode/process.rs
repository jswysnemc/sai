use anyhow::{bail, Context, Result};
use sai_codemode::protocol::{
    ExecutionRequest, ToolReply, WorkerEvent, MAX_BRIDGE_BYTES, MAX_FRAME_BYTES, WORKER_ARG,
};
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

pub(crate) struct Worker {
    child: Child,
    input: mpsc::Sender<Vec<u8>>,
    output: mpsc::Receiver<Result<WorkerEvent>>,
    reader: tokio::task::JoinHandle<()>,
    writer: tokio::task::JoinHandle<()>,
    sent_bytes: usize,
}

impl Worker {
    /// 【CodeMode】【进程启动】使用 executable 启动隔离进程并交付 request，返回通信句柄。
    pub(crate) async fn start(executable: &Path, request: &ExecutionRequest) -> Result<Self> {
        let mut child = Command::new(executable)
            .arg(WORKER_ARG)
            .env_clear()
            .current_dir(std::env::temp_dir())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .context("failed to launch codemode worker")?;
        let mut stdin = child.stdin.take().context("missing worker stdin")?;
        let mut stdout = BufReader::new(child.stdout.take().context("missing worker stdout")?);
        let (input, mut input_rx) = mpsc::channel::<Vec<u8>>(128);
        let (output_tx, output) = mpsc::channel(16);
        // 1. 【CodeMode】【双向通信】读写独立调度，避免并行请求与大结果互相占满管道
        let writer = tokio::spawn(async move {
            while let Some(frame) = input_rx.recv().await {
                if stdin.write_all(&frame).await.is_err() || stdin.flush().await.is_err() {
                    break;
                }
            }
        });
        let reader = tokio::spawn(async move {
            let mut received_bytes = 0usize;
            loop {
                let result = async {
                    let mut frame = Vec::new();
                    let mut bounded = (&mut stdout).take((MAX_FRAME_BYTES + 1) as u64);
                    bounded.read_until(b'\n', &mut frame).await?;
                    received_bytes = received_bytes.saturating_add(frame.len());
                    if received_bytes > MAX_BRIDGE_BYTES {
                        bail!("codemode total IPC traffic exceeds size limit");
                    }
                    if frame.len() > MAX_FRAME_BYTES {
                        bail!("codemode IPC frame exceeds size limit");
                    }
                    if frame.last() != Some(&b'\n') {
                        bail!("codemode worker exited or crashed before completion");
                    }
                    Ok(serde_json::from_slice(&frame)?)
                }
                .await;
                let done = result.is_err();
                if output_tx.send(result).await.is_err() || done {
                    break;
                }
            }
        });
        let mut worker = Self {
            child,
            input,
            output,
            reader,
            writer,
            sent_bytes: 0,
        };
        worker.send(request).await?;
        Ok(worker)
    }

    /// 【CodeMode】【回复交付】把 reply 发送给工作进程，返回发送结果。
    pub(crate) async fn reply(&mut self, reply: &ToolReply) -> Result<()> {
        self.send(reply).await
    }

    /// 【CodeMode】【帧发送】编码 value 并发送容量受限帧，返回发送结果。
    async fn send(&mut self, value: &impl serde::Serialize) -> Result<()> {
        let mut frame = serde_json::to_vec(value)?;
        if frame.len() + 1 > MAX_FRAME_BYTES {
            bail!("codemode IPC frame exceeds size limit");
        }
        frame.push(b'\n');
        self.sent_bytes = self.sent_bytes.saturating_add(frame.len());
        if self.sent_bytes > MAX_BRIDGE_BYTES {
            bail!("codemode total IPC traffic exceeds size limit");
        }
        self.input
            .send(frame)
            .await
            .context("codemode worker input channel closed")
    }

    /// 【CodeMode】【事件读取】读取一个容量受限事件，返回脚本输出或工具请求。
    pub(crate) async fn next(&mut self) -> Result<WorkerEvent> {
        self.output
            .recv()
            .await
            .context("codemode worker output channel closed")?
    }

    /// 【CodeMode】【进程回收】停止并等待工作进程，返回回收结果。
    pub(crate) async fn stop(&mut self) -> Result<()> {
        if self.child.try_wait()?.is_none() {
            self.child.kill().await?;
        }
        Ok(())
    }
}

impl Drop for Worker {
    /// 【CodeMode】【通信清理】无参数；终止 IO 任务，Child 的 kill_on_drop 回收工作进程。
    fn drop(&mut self) {
        self.reader.abort();
        self.writer.abort();
    }
}
