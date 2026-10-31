use crate::protocol::{self, ExecutionRequest, ToolReply, WorkerEvent};
use crate::store;
use anyhow::{bail, Result};
use rquickjs::{CaughtError, Context, Function, Object, Promise, Runtime};
use std::cell::RefCell;
#[cfg(test)]
use std::io::{BufRead, Write};
use std::sync::mpsc::{sync_channel, SyncSender};
use std::time::{Duration, Instant};

/// 【CodeMode】【工作进程】从标准输入执行一次请求，把工具调用和结果写到标准输出。
/// 返回协议或运行时初始化错误；脚本错误通过 Done 帧报告。
pub fn serve() -> Result<()> {
    let mut input = std::io::stdin().lock();
    let request = protocol::read_frame(&mut input)?;
    execute(
        request,
        |event| protocol::write_frame(&mut std::io::stdout().lock(), &event),
        &mut || protocol::read_frame(&mut input),
    )
}

/// 【CodeMode】【脚本执行】执行 request，send 交付事件，receive 提供宿主回复，返回运行结果。
/// 同步死循环使用引擎中断，主进程另设硬超时与崩溃隔离边界。
pub fn execute(
    request: ExecutionRequest,
    mut send: impl FnMut(WorkerEvent) -> Result<()> + Send + 'static,
    receive: &mut impl FnMut() -> Result<ToolReply>,
) -> Result<()> {
    let (sender, events) = sync_channel(16);
    // 1. 【CodeMode】【输出隔离】专用写线程持续刷新有界通道，同步死循环不阻断先前输出
    let writer = std::thread::spawn(move || -> Result<()> {
        for event in events {
            send(event)?;
        }
        Ok(())
    });
    let result = execute_vm(request, &sender, receive);
    drop(sender);
    writer
        .join()
        .map_err(|_| anyhow::anyhow!("codemode output writer panicked"))??;
    result
}

/// 【CodeMode】【解释器执行】运行 request，sender 交付事件，receive 提供工具回复，返回执行结果。
fn execute_vm(
    request: ExecutionRequest,
    sender: &SyncSender<WorkerEvent>,
    receive: &mut impl FnMut() -> Result<ToolReply>,
) -> Result<()> {
    if request.code.len() > protocol::MAX_CODE_BYTES || request.timeout_ms == 0 {
        bail!("invalid codemode execution limits");
    }
    store::validate(&request.store)?;
    let runtime = Runtime::new()?;
    runtime.set_memory_limit(256 * 1024 * 1024);
    runtime.set_max_stack_size(1024 * 1024);
    let deadline = Instant::now() + Duration::from_millis(request.timeout_ms.min(600_000));
    runtime.set_interrupt_handler(Some(Box::new(move || Instant::now() >= deadline)));
    let context = Context::full(&runtime)?;
    let result = context.with(|ctx| -> Result<()> {
        let callback = sender.clone();
        let usage = RefCell::new((0usize, 0usize));
        let emit = Function::new(ctx.clone(), move |json: String| -> rquickjs::Result<()> {
            let event: WorkerEvent =
                serde_json::from_str(&json).map_err(|_| rquickjs::Error::Unknown)?;
            if let WorkerEvent::Output { value, .. } = &event {
                let mut usage = usage.borrow_mut();
                usage.0 = usage.0.saturating_add(value.len());
                usage.1 += 1;
                if usage.0 > crate::output::MAX_OUTPUT_BYTES
                    || usage.1 > crate::output::MAX_OUTPUT_ITEMS
                {
                    return Err(rquickjs::Error::Unknown);
                }
            }
            // 1. 【CodeMode】【立即交付】同步计算开始前刷新输出，主进程强制截止也能保留部分结果
            callback.send(event).map_err(|_| rquickjs::Error::Unknown)
        })?;
        let factory: Function = ctx.eval(include_str!("prelude.js"))?;
        let controller: Object = factory
            .call((emit, serde_json::to_string(&request)?))
            .map_err(|error| anyhow::anyhow!("{}", CaughtError::from_error(&ctx, error)))?;
        let run: Function = controller.get("run")?;
        let resolve: Function = controller.get("resolve")?;
        let pending_count: Function = controller.get("pendingCount")?;
        let promise: Promise = run.call((request.code,)).map_err(|error| {
            if Instant::now() >= deadline {
                anyhow::anyhow!("codemode script timed out")
            } else {
                anyhow::anyhow!("{}", CaughtError::from_error(&ctx, error))
            }
        })?;
        loop {
            if Instant::now() >= deadline {
                bail!("codemode script timed out");
            }
            if let Some(result) = promise.result::<String>() {
                let json = result
                    .map_err(|error| anyhow::anyhow!("{}", CaughtError::from_error(&ctx, error)))?;
                let completion: serde_json::Value = serde_json::from_str(&json)?;
                let writes = serde_json::from_value(completion["writes"].clone())?;
                sender.send(WorkerEvent::Done {
                    error: completion["error"].as_str().map(str::to_owned),
                    writes,
                })?;
                return Ok(());
            }
            // 2. 【CodeMode】【微任务调度】无微任务但存在工具请求时等待宿主，否则判定停滞
            if ctx.execute_pending_job() {
                continue;
            }
            let pending: usize = pending_count.call(())?;
            if pending == 0 {
                bail!("codemode script stalled: unresolved promise with no pending tool calls");
            }
            let reply = receive()?;
            resolve.call::<_, ()>((serde_json::to_string(&reply)?,))?;
        }
    });
    if let Err(error) = result {
        sender.send(WorkerEvent::Done {
            error: Some(error.to_string()),
            writes: Default::default(),
        })?;
    }
    Ok(())
}

/// 【CodeMode】【帧测试】用内存输入输出执行工作进程协议，返回输出字节。
#[cfg(test)]
pub(crate) fn execute_stream(
    request: ExecutionRequest,
    input: &mut impl BufRead,
) -> Result<Vec<u8>> {
    let output = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = output.clone();
    execute(
        request,
        move |event| protocol::write_frame(&mut *sink.lock().unwrap(), &event),
        &mut || protocol::read_frame(input),
    )?;
    let mut bytes = std::mem::take(&mut *output.lock().unwrap());
    bytes.flush()?;
    Ok(bytes)
}
