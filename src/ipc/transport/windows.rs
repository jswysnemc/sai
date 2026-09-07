use super::*;
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeServer, ServerOptions};
use tokio::sync::Mutex;

/// `ERROR_NO_DATA`：客户端连上后没写任何数据就断开。
const ERROR_NO_DATA: i32 = 232;
/// `ERROR_PIPE_NOT_CONNECTED`：管道实例已断开。
const ERROR_PIPE_NOT_CONNECTED: i32 = 233;
/// `ERROR_ACCESS_DENIED`：`first_pipe_instance(true)` 撞上已存在的同名管道。
const ERROR_ACCESS_DENIED: i32 = 5;
/// `ERROR_PIPE_BUSY`：管道实例数被打满。
const ERROR_PIPE_BUSY: i32 = 231;

/// named pipe 传输层。
///
/// Windows 上没有「socket 文件」，所以无法像 Unix 那样靠文件是否存在判断残留。
/// 独占性靠 [`ServerOptions::first_pipe_instance`] 保证：名字已被占用时
/// `create` 直接失败，语义上等价于 Unix 的 `bind` 撞 `AddrInUse`。
pub(super) struct WinPipeTransport {
    name: String,
    /// 已创建但还没等到客户端的管道实例。
    ///
    /// 管道归传输层所有，取消 accept 等待时也必须保留，避免心跳分支关闭连接
    pending: Mutex<Option<NamedPipeServer>>,
    /// 本进程是否是持有者。
    holder: bool,
}

impl WinPipeTransport {
    /// 【IPC】【管道绑定】尝试取得首个实例，区分持有者和观察者。
    /// 参数：`state_dir` 为会话状态目录；返回：传输端点或创建错误。
    pub(super) fn bind(state_dir: &Path) -> Result<Self> {
        let name = pipe_name(state_dir);

        match ServerOptions::new().first_pipe_instance(true).create(&name) {
            Ok(server) => Ok(Self {
                name,
                pending: Mutex::new(Some(server)),
                holder: true,
            }),
            Err(err) if is_pipe_taken(&err) => {
                // 名字已被占用：本进程作为观察者接入，绝不能抢持有者的管道。
                Ok(Self {
                    name,
                    pending: Mutex::new(None),
                    holder: false,
                })
            }
            Err(err) => Err(err).with_context(|| format!("创建 named pipe {} 失败", name)),
        }
    }

    /// 【IPC】【管道实例】创建后续监听实例。
    /// 参数：无；返回：命名管道实例或系统错误。
    fn create_instance(&self) -> std::io::Result<NamedPipeServer> {
        ServerOptions::new().create(&self.name)
    }
}

/// 【IPC】【管道占用】判断同名管道是否已由其他进程持有。
/// 参数：`err` 为创建错误；返回：是否应以观察者身份连接。
fn is_pipe_taken(err: &std::io::Error) -> bool {
    matches!(
        err.raw_os_error(),
        Some(ERROR_ACCESS_DENIED) | Some(ERROR_PIPE_BUSY)
    )
}

/// 脏实例：探活（`probe_holder`）建连后立刻断开，会在持有者侧留下一个
/// 已经断连的管道实例，需要丢掉重来。
/// 参数：`err` 为连接错误；返回：是否需要替换实例后重试。
fn is_stale_pipe_error(err: &std::io::Error) -> bool {
    matches!(
        err.raw_os_error(),
        Some(ERROR_NO_DATA) | Some(ERROR_PIPE_NOT_CONNECTED)
    )
}

#[async_trait]
impl SessionTransport for WinPipeTransport {
    /// 返回命名管道端点，无参数，返回端点名称。
    fn endpoint(&self) -> Endpoint {
        Endpoint::WinPipe(self.name.clone())
    }

    /// 返回当前进程是否持有监听端点，无参数，返回持有者标记。
    fn is_holder(&self) -> bool {
        self.holder
    }

    /// 【IPC】【连接等待】接受观察者连接，取消等待时保留待交接管道。
    /// 参数：无；返回：可双向收发的流或连接错误。
    async fn accept(&self) -> Result<Box<dyn SessionStream>> {
        if !self.holder {
            return Err(anyhow!(
                "本进程不是 ipc 持有者（{} 上已有其他进程在监听），不能以观察者身份 accept",
                self.name
            ));
        }

        // 1. 【IPC】【连接等待】只借用管道等待连接，取消 future 时释放锁而不关闭管道
        let mut pending = self.pending.lock().await;
        loop {
            let server = pending.as_ref().context("持有者缺少待连接管道")?;
            match server.connect().await {
                Ok(()) => {
                    // 2. 【IPC】【连接交接】先建立下一个监听实例，再移交已连接管道
                    let next = self
                        .create_instance()
                        .context("创建后续 named pipe 实例失败")?;
                    let connected = pending.replace(next).context("已连接管道丢失")?;
                    return Ok(Box::new(FramedStream::new(connected)));
                }
                Err(err) if is_stale_pipe_error(&err) => {
                    // 3. 【IPC】【连接重试】探活留下的断连实例需替换后继续等待
                    let next = self
                        .create_instance()
                        .context("替换断连 named pipe 实例失败")?;
                    *pending = Some(next);
                }
                Err(err) => return Err(err).context("等待观察者连接失败"),
            }
        }
    }

    /// 【IPC】【观察者连接】打开持有者的命名管道。
    /// 参数：无；返回：可双向收发的流或连接错误。
    async fn connect(&self) -> Result<Box<dyn SessionStream>> {
        let client = ClientOptions::new()
            .open(&self.name)
            .with_context(|| format!("连接 named pipe {} 失败", self.name))?;
        Ok(Box::new(FramedStream::new(client)))
    }
}

#[cfg(test)]
#[path = "windows_tests.rs"]
mod tests;
