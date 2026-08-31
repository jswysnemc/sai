//! 跨平台 IPC 传输层。
//!
//! 两个 sai 进程（TUI 进程 `sai` 与 Web 进程 `sai web`）共享同一个会话的 `state_dir`，
//! 其中一个是**持有者**（唯一持有 Agent、驱动轮次），另一个是**观察者**（订阅事件流、
//! 提交消息、响应权限请求）。传输层负责在两者之间提供一条本地双向字节流：
//!
//! - Unix：`{state_dir}/sai-bus-{h8}` 上的 Unix domain socket
//! - Windows：`\\.\pipe\sai-{h8}` 上的 named pipe
//!
//! `h8` 是 `blake3(canonical(state_dir))` 的前 8 个十六进制字符。加哈希有两个原因：
//! macOS 上 UDS 路径有 104 字节的硬上限，直接把 state_dir 拼进去很容易超；
//! 同时同一个 state_dir 需要稳定映射到同一个端点，否则观察者找不到持有者。
//!
//! P1 只交付传输与端点管理，不含事件订阅、重连退避、SessionActor 接入。
//! 那些是 P3/P4 的内容，届时这里的大部分 API 才会有真正的调用方。

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::io::{AsyncRead, AsyncWrite, BufStream};

use crate::ipc::frame::{read_frame, write_frame, Frame};

/// 传输端点。
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum Endpoint {
    /// Unix domain socket 路径
    // P3/P4 接入后观察者侧会读这个字段拼日志与报错。
    #[allow(dead_code)]
    Unix(PathBuf),
    /// Windows named pipe 名（形如 `\\.\pipe\sai-xxxxxxxx`）
    // P3/P4 接入后观察者侧会读这个字段拼日志与报错。
    #[allow(dead_code)]
    WinPipe(String),
}

/// 传输端点上已建立的一条双向连接：按 [`Frame`] 收发。
#[async_trait]
pub(crate) trait SessionStream: Send {
    /// 写一帧并 flush。
    // P3/P4 接入后观察者与持有者的收发循环会调用。
    #[allow(dead_code)]
    async fn send(&mut self, frame: &Frame) -> Result<()>;

    /// 读一帧；对端干净关闭（EOF）返回 `Ok(None)`。
    // P3/P4 接入后观察者与持有者的收发循环会调用。
    #[allow(dead_code)]
    async fn recv(&mut self) -> Result<Option<Frame>>;
}

/// 端点管理：持有者侧 `accept`，观察者侧 `connect`。
// `#[async_trait]` 是必须的：`transport_for_state_dir` 要返回 `Box<dyn SessionTransport>`，
// 而带 `async fn` 的原生 trait 不是 dyn 兼容的，编译不过。
#[async_trait]
pub(crate) trait SessionTransport: Send + Sync {
    fn endpoint(&self) -> Endpoint;

    /// 本进程是否拿到了持有者租约。
    ///
    /// 这是「谁是持有者」的唯一原子仲裁者：Unix 上是 `flock`，Windows 上是
    /// `first_pipe_instance`。持有者登记表是「读 + 写」两步，两个进程同时探测到
    /// 「没有持有者」时两边都能写成功，不能拿它来仲裁。
    /// P4 的持有者选举与接管看门狗都以此为准。
    fn is_holder(&self) -> bool;

    /// 接受一个观察者连接（持有者侧）。
    ///
    /// 本进程不是持有者时（`state_dir` 上已经有别的进程在监听）返回错误——
    /// 此时应该走 [`Self::connect`] 以观察者身份接入，而不是抢监听。
    // P3/P4 接入后持有者的 accept 循环会调用。
    #[allow(dead_code)]
    async fn accept(&self) -> Result<Box<dyn SessionStream>>;

    /// 连接到持有者（观察者侧）。
    // P3/P4 接入后观察者建连会调用。
    #[allow(dead_code)]
    async fn connect(&self) -> Result<Box<dyn SessionStream>>;
}

/// 把任意 `AsyncRead + AsyncWrite` 包装成按帧收发的 [`SessionStream`]。
///
/// 用 [`BufStream`] 而不是裸流：读侧需要 `AsyncBufRead` 才能做 `fill_buf`/`consume`
/// 的增量分帧（见 `frame::read_frame`），写侧需要缓冲以避免每个小帧都打一次 syscall。
pub(crate) struct FramedStream<RW> {
    inner: BufStream<RW>,
}

impl<RW> FramedStream<RW>
where
    RW: AsyncRead + AsyncWrite + Unpin,
{
    // P3/P4 接入后 unix/windows 两个平台的 accept/connect 会调用。
    #[allow(dead_code)]
    pub(crate) fn new(inner: RW) -> Self {
        Self {
            inner: BufStream::new(inner),
        }
    }
}

#[async_trait]
impl<RW> SessionStream for FramedStream<RW>
where
    RW: AsyncRead + AsyncWrite + Send + Unpin,
{
    async fn send(&mut self, frame: &Frame) -> Result<()> {
        write_frame(&mut self.inner, frame).await
    }

    async fn recv(&mut self) -> Result<Option<Frame>> {
        read_frame(&mut self.inner).await
    }
}

/// `state_dir` 的稳定短哈希（blake3 前 8 个十六进制字符）。
fn endpoint_hash(state_dir: &Path) -> String {
    // canonicalize 消掉 `..`、软链和 `./` 前缀，保证不同写法指向同一目录时得到同一个哈希。
    // 目录还不存在时（理论上持有者启动前 state_dir 应该已经建好）退化成原路径，
    // 至少保证同一进程内前后一致。
    let canonical = state_dir
        .canonicalize()
        .unwrap_or_else(|_| state_dir.to_path_buf());

    let mut hasher = blake3::Hasher::new();
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        hasher.update(canonical.as_os_str().as_bytes());
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        for unit in canonical.as_os_str().encode_wide() {
            hasher.update(&unit.to_le_bytes());
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        hasher.update(canonical.to_string_lossy().as_bytes());
    }
    hasher.finalize().to_hex()[..8].to_string()
}

/// 本平台的 socket 文件路径：`{state_dir}/sai-bus-{h8}`。
#[cfg(unix)]
fn socket_path(state_dir: &Path) -> PathBuf {
    state_dir.join(format!("sai-bus-{}", endpoint_hash(state_dir)))
}

/// 本平台的 named pipe 名：`\\.\pipe\sai-{h8}`。
///
/// 管道名里不能出现路径分隔符，所以只取哈希、不取 state_dir 本身。
#[cfg(windows)]
fn pipe_name(state_dir: &Path) -> String {
    format!(r"\\.\pipe\sai-{}", endpoint_hash(state_dir))
}

/// 按 `state_dir` 推导端点。
///
/// 目前只被平台无关的测试（端点推导）使用；两个平台的实现直接走哈希与套接字
/// 路径构造，避免哈希逻辑出现两份。
#[cfg(test)]
fn endpoint_for(state_dir: &Path) -> Endpoint {
    #[cfg(unix)]
    return Endpoint::Unix(socket_path(state_dir));
    #[cfg(windows)]
    return Endpoint::WinPipe(pipe_name(state_dir));
    #[cfg(not(any(unix, windows)))]
    {
        let _ = state_dir;
        unreachable!("sai 的 ipc 传输层只支持 unix 与 windows")
    }
}

// ---------------------------------------------------------------------------
// Unix：Unix domain socket
// ---------------------------------------------------------------------------

#[cfg(unix)]
mod unix {
    use super::*;
    use std::fs::{File, OpenOptions};
    use tokio::net::{UnixListener, UnixStream};

    /// Unix domain socket 传输层。
    ///
    /// `listener` 为 `Some` 表示本进程是持有者；为 `None` 表示 `bind` 时探测到
    /// 已有别的进程在监听，本进程只能作为观察者接入。
    pub(super) struct UnixTransport {
        path: PathBuf,
        /// 持有者租约锁。持有者模式下 `Some`，进程存活期间一直持有。
        ///
        /// 这个字段只为了把锁活到传输层析构——`flock` 在 fd 关闭时自动释放，
        /// 进程崩溃时由内核释放，不需要任何清理逻辑。
        ///
        /// 它从不参与任何判断，纯 RAII 占位，所以是 dead_code。
        #[allow(dead_code)]
        lease: Option<File>,
        listener: Option<UnixListener>,
    }

    impl UnixTransport {
        pub(super) fn bind(state_dir: &Path) -> Result<Self> {
            let path = socket_path(state_dir);

            // 先抢「持有者租约」再 bind。用 flock 而不是「试着 connect 一下」来判断
            // 有没有活着的持有者，有两个原因：
            //   1. connect 探活会在持有者的 accept 队列里留下一个建连后立刻断开的
            //      幽灵连接，持有者 accept 出来只能读到 EOF；
            //   2. flock 随 fd 关闭/进程退出自动释放，崩溃残留也能正确判定，
            //      而「socket 文件还在」本身并不能说明对端活着。
            let lease = OpenOptions::new()
                .create(true)
                .write(true)
                .open(lease_path(state_dir))
                .with_context(|| {
                    format!("打开 ipc 持有者租约锁失败（state_dir: {}）", state_dir.display())
                })?;

            // 抢不到租约 = 有活的持有者：绝不动它的端点，本进程作为观察者接入。
            if lease.try_lock().is_err() {
                return Ok(Self {
                    path,
                    lease: None,
                    listener: None,
                });
            }

            // 拿到租约就意味着端点上不可能有活的持有者，此时残留的 socket 文件
            // 一定是上次崩溃留下的垃圾，清理掉再 bind 是安全的。
            // （bind 本身仍可能因为竞态报 AddrInUse，那种情况直接向上抛错，
            //   不重试也不覆盖——宁可让调用方看到明确的失败。）
            let _ = std::fs::remove_file(&path);
            let listener = UnixListener::bind(&path)
                .with_context(|| format!("绑定 ipc socket {} 失败", path.display()))?;
            Ok(Self {
                path,
                lease: Some(lease),
                listener: Some(listener),
            })
        }

        /// 取出监听句柄；观察者模式下返回错误。
        fn listener(&self) -> Result<&UnixListener> {
            self.listener.as_ref().ok_or_else(|| {
                anyhow!(
                    "本进程不是 ipc 持有者（{} 上已有其他进程在监听），不能以观察者身份 accept",
                    self.path.display()
                )
            })
        }
    }

    /// 持有者退出时清理 socket 文件，避免留下残留文件让下一次 bind 走清理分支。
    ///
    /// 只删自己成功 bind 的那个路径（`listener` 为 `Some`），观察者模式下绝不删别人的
    /// 监听文件。租约锁文件**故意不删**：删掉会让它和重新 create 出来的新 inode 变成
    /// 两把不同的锁，两个持有者可能同时拿到租约。留一个 0 字节文件是安全的。
    impl Drop for UnixTransport {
        fn drop(&mut self) {
            if self.listener.is_some() {
                let _ = std::fs::remove_file(&self.path);
            }
        }
    }

    /// 持有者租约锁文件路径：`{state_dir}/sai-bus-{h8}.lease`。
    fn lease_path(state_dir: &Path) -> PathBuf {
        let mut path = socket_path(state_dir).into_os_string();
        path.push(".lease");
        PathBuf::from(path)
    }

    /// 打开已存在的租约锁文件；从没有进程成为过持有者时返回 `None`。
    ///
    /// 刻意不带 `create(true)`：探活不该在 state_dir 里留下新文件。
    pub(super) fn open_lease(state_dir: &Path) -> Option<File> {
        OpenOptions::new().write(true).open(lease_path(state_dir)).ok()
    }

    #[async_trait]
    impl SessionTransport for UnixTransport {
        fn endpoint(&self) -> Endpoint {
            Endpoint::Unix(self.path.clone())
        }

        fn is_holder(&self) -> bool {
            // 抢到租约才会去 bind，监听器在就等价于持有者
            self.listener.is_some()
        }

        async fn accept(&self) -> Result<Box<dyn SessionStream>> {
            let listener = self.listener()?;
            let (stream, _addr) = listener.accept().await.context("接受观察者连接失败")?;
            Ok(Box::new(FramedStream::new(stream)))
        }

        async fn connect(&self) -> Result<Box<dyn SessionStream>> {
            let stream = UnixStream::connect(&self.path)
                .await
                .with_context(|| format!("连接 ipc socket {} 失败", self.path.display()))?;
            Ok(Box::new(FramedStream::new(stream)))
        }
    }
}

// ---------------------------------------------------------------------------
// Windows：named pipe
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod windows {
    use super::*;
    use std::sync::Mutex;
    use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeServer, ServerOptions};

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
        /// 持有者模式下恒定保持一个待连接实例，否则观察者 `open` 的瞬间可能落到
        /// `NotFound`（见 tokio `named_pipe` 模块文档里「correctly implemented server」）。
        pending: Mutex<Option<NamedPipeServer>>,
        /// 本进程是否是持有者。
        holder: bool,
    }

    impl WinPipeTransport {
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

        /// 创建一个**非首个**管道实例。
        fn create_instance(&self) -> std::io::Result<NamedPipeServer> {
            ServerOptions::new().create(&self.name)
        }

        fn take_pending(&self) -> Option<NamedPipeServer> {
            self.pending
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .take()
        }

        fn set_pending(&self, server: Option<NamedPipeServer>) {
            *self.pending.lock().unwrap_or_else(|err| err.into_inner()) = server;
        }
    }

    fn is_pipe_taken(err: &std::io::Error) -> bool {
        matches!(
            err.raw_os_error(),
            Some(ERROR_ACCESS_DENIED) | Some(ERROR_PIPE_BUSY)
        )
    }

    /// 脏实例：探活（`probe_holder`）建连后立刻断开，会在持有者侧留下一个
    /// 已经断连的管道实例，需要丢掉重来。
    fn is_stale_pipe_error(err: &std::io::Error) -> bool {
        matches!(
            err.raw_os_error(),
            Some(ERROR_NO_DATA) | Some(ERROR_PIPE_NOT_CONNECTED)
        )
    }

    #[async_trait]
    impl SessionTransport for WinPipeTransport {
        fn endpoint(&self) -> Endpoint {
            Endpoint::WinPipe(self.name.clone())
        }

        fn is_holder(&self) -> bool {
            self.holder
        }

        async fn accept(&self) -> Result<Box<dyn SessionStream>> {
            if !self.holder {
                return Err(anyhow!(
                    "本进程不是 ipc 持有者（{} 上已有其他进程在监听），不能以观察者身份 accept",
                    self.name
                ));
            }

            loop {
                let server = match self.take_pending() {
                    Some(server) => server,
                    None => self
                        .create_instance()
                        .with_context(|| format!("创建 named pipe 实例 {} 失败", self.name))?,
                };
                // 先备好下一个实例再去等这个实例连上，避免观察者落进 NotFound 窗口。
                self.set_pending(self.create_instance().ok());

                match server.connect().await {
                    Ok(()) => return Ok(Box::new(FramedStream::new(server))),
                    // 每轮都会消耗掉一个实例，脏实例有限，循环不会空转。
                    Err(err) if is_stale_pipe_error(&err) => continue,
                    Err(err) => return Err(err).context("等待观察者连接失败"),
                }
            }
        }

        async fn connect(&self) -> Result<Box<dyn SessionStream>> {
            let client = ClientOptions::new()
                .open(&self.name)
                .with_context(|| format!("连接 named pipe {} 失败", self.name))?;
            Ok(Box::new(FramedStream::new(client)))
        }
    }
}

// ---------------------------------------------------------------------------
// 平台入口
// ---------------------------------------------------------------------------

/// 按 `state_dir` 推导端点并构造传输层。
///
/// 返回 `Ok` 有两种含义，用 [`SessionTransport::accept`] 区分：
/// `accept` 成功说明本进程是持有者；返回「已有其他进程在监听」的错误说明
/// `state_dir` 上已有持有者，本进程应改走 [`SessionTransport::connect`]。
///
/// **先探活再 bind**：`bind` 之前先抢一次持有者租约锁（Unix 上是 `flock`，
/// Windows 上是 `first_pipe_instance`）。抢不到说明已有持有者，此时**不会**清理或
/// 抢占对端的端点；只有确认没有持有者时，才会认为残留的是上次崩溃留下的垃圾，
/// 清理后重新 bind。
// P3/P4 接入会话启动流程后会有调用方。
#[allow(dead_code)]
#[cfg(unix)]
pub(crate) fn transport_for_state_dir(state_dir: &Path) -> Result<Box<dyn SessionTransport>> {
    Ok(Box::new(unix::UnixTransport::bind(state_dir)?))
}

/// 按 `state_dir` 推导端点并构造传输层。
///
/// 语义同 Unix 版本，见上面的说明。
// P3/P4 接入会话启动流程后会有调用方。
#[allow(dead_code)]
#[cfg(windows)]
pub(crate) fn transport_for_state_dir(state_dir: &Path) -> Result<Box<dyn SessionTransport>> {
    Ok(Box::new(windows::WinPipeTransport::bind(state_dir)?))
}

/// 探测是否已有持有者在监听。
///
/// 实现上是「能不能抢到持有者租约锁」，而不是真的去 `connect`：connect 会在持有者的
/// accept 队列里留下一个建连后立刻断开的幽灵连接，而租约锁既能准确反映「有没有活着的
/// 持有者」（进程崩溃时由内核自动释放），又完全没有副作用。
// P3/P4 接入会话启动流程后会有调用方。
#[allow(dead_code)]
#[cfg(unix)]
pub(crate) async fn probe_holder(state_dir: &Path) -> bool {
    // 抢不到租约 = 有活的持有者占着。
    unix::open_lease(state_dir).is_some_and(|lease| lease.try_lock().is_err())
}

/// 探测是否已有持有者在监听。
///
/// 语义同 Unix 版本。Windows 上管道名被占用时 `ClientOptions::open` 会失败，
/// 判定是准确的；代价是探活成功会在持有者侧留下一个建连后立刻断开的管道实例，
/// 由 `accept` 里的脏实例重试逻辑消化掉。
// P3/P4 接入会话启动流程后会有调用方。
#[allow(dead_code)]
#[cfg(windows)]
pub(crate) async fn probe_holder(state_dir: &Path) -> bool {
    use tokio::net::windows::named_pipe::ClientOptions;
    let name = pipe_name(state_dir);
    ClientOptions::new().open(&name).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn ev(sequence: u64, text: &str) -> Frame {
        Frame {
            kind: crate::ipc::frame::KIND_EVT_RUNNER.to_string(),
            sequence: Some(sequence),
            payload: json!({"text": text}),
        }
    }

    /// 端点推导：同一 state_dir 必须稳定映射到同一端点。
    #[test]
    fn endpoint_is_stable_for_same_state_dir() {
        let dir = TempDir::new().unwrap();
        assert_eq!(endpoint_for(dir.path()), endpoint_for(dir.path()));

        // 同一目录的不同写法（带尾部分隔符）也必须落到同一端点。
        let alt = dir.path().join(".");
        assert_eq!(endpoint_for(dir.path()), endpoint_for(&alt));
    }

    /// 端点推导：不同 state_dir 必须得到不同端点，否则观察者会连到别人的会话上。
    #[test]
    fn endpoint_differs_across_state_dirs() {
        let a = TempDir::new().unwrap();
        let b = TempDir::new().unwrap();
        assert_ne!(endpoint_for(a.path()), endpoint_for(b.path()));
    }

    /// 端点名必须是「短哈希」形态（Unix）：socket 文件名是 `sai-bus-{8 位十六进制}`。
    ///
    /// 按平台拆成两个测试而不是在一个 match 里兜底：另一个平台的变体在本平台不可达，
    /// 兜底 arm 会触发 `unreachable_patterns`。
    #[cfg(unix)]
    #[test]
    fn endpoint_name_shape() {
        let dir = TempDir::new().unwrap();
        let path = socket_path(dir.path());
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let hash = name
            .strip_prefix("sai-bus-")
            .unwrap_or_else(|| panic!("socket 名应带 sai-bus- 前缀，实际: {name}"));
        assert_eq!(hash.len(), 8, "h8 应是 8 个字符，实际: {hash}");
        assert!(
            hash.chars().all(|c| c.is_ascii_hexdigit()),
            "h8 应是十六进制，实际: {hash}"
        );
        // socket 必须落在 state_dir 里，否则观察者找不到。
        assert_eq!(path.parent(), Some(dir.path()));
    }

    /// 端点名必须是「短哈希」形态（Windows）：管道名里不能出现路径分隔符。
    #[cfg(windows)]
    #[test]
    fn endpoint_name_shape() {
        let dir = TempDir::new().unwrap();
        let name = pipe_name(dir.path());
        // `\\.\pipe\` 前缀之后的主体里出现分隔符会让 Windows 直接拒绝创建管道。
        let body = &name[r"\\.\pipe\".len()..];
        assert!(!body.contains(['/', '\\']), "管道名含分隔符: {name}");
        assert_eq!(name.len(), r"\\.\pipe\sai-".len() + 8);
    }

    /// 往返：两个 FramedStream 互发若干帧，顺序与内容都必须一致。
    #[tokio::test]
    async fn roundtrip_preserves_order_and_content() -> Result<()> {
        let (a, b) = tokio::io::duplex(4096);
        let mut a = FramedStream::new(a);
        let mut b = FramedStream::new(b);

        for i in 0..64u64 {
            a.send(&ev(i, &format!("a{i}"))).await?;
            b.send(&ev(i, &format!("b{i}"))).await?;
        }

        for i in 0..64u64 {
            assert_eq!(b.recv().await?, Some(ev(i, &format!("a{i}"))));
            assert_eq!(a.recv().await?, Some(ev(i, &format!("b{i}"))));
        }
        Ok(())
    }

    /// 往返：控制帧（无 sequence）也要原样回来。
    #[tokio::test]
    async fn control_frames_roundtrip() -> Result<()> {
        let (a, b) = tokio::io::duplex(1024);
        let mut a = FramedStream::new(a);
        let mut b = FramedStream::new(b);

        let ping = Frame::control(crate::ipc::frame::KIND_CTL_PING, json!({"t": 1}));
        a.send(&ping).await?;
        assert_eq!(b.recv().await?, Some(ping));
        Ok(())
    }

    /// 监听与连接：持有者 bind 后观察者能连上，并双向收发。
    #[tokio::test]
    async fn holder_accepts_and_talks_to_observer() -> Result<()> {
        let dir = TempDir::new().unwrap();
        let holder = transport_for_state_dir(dir.path())?;

        let observer = transport_for_state_dir(dir.path())?;
        let mut observer_stream = observer.connect().await?;
        let mut holder_stream = holder.accept().await?;

        // 观察者 -> 持有者
        for i in 0..16u64 {
            observer_stream.send(&ev(i, &format!("up{i}"))).await?;
        }
        for i in 0..16u64 {
            assert_eq!(holder_stream.recv().await?, Some(ev(i, &format!("up{i}"))));
        }

        // 持有者 -> 观察者
        for i in 0..16u64 {
            holder_stream.send(&ev(i, &format!("down{i}"))).await?;
        }
        for i in 0..16u64 {
            assert_eq!(observer_stream.recv().await?, Some(ev(i, &format!("down{i}"))));
        }
        Ok(())
    }

    /// 探活：没有监听时返回 false。
    #[tokio::test]
    async fn probe_is_false_without_holder() {
        let dir = TempDir::new().unwrap();
        assert!(!probe_holder(dir.path()).await);
    }

    /// 探活：有监听时返回 true。
    #[tokio::test]
    async fn probe_is_true_with_holder() -> Result<()> {
        let dir = TempDir::new().unwrap();
        assert!(!probe_holder(dir.path()).await);

        let _holder = transport_for_state_dir(dir.path())?;
        assert!(probe_holder(dir.path()).await);
        Ok(())
    }

    /// 探活必须真的去连，而不是只看端点文件/名字存不存在：
    /// 残留的 socket 文件不该被误判成有持有者。
    #[tokio::test]
    #[cfg(unix)]
    async fn probe_is_false_for_stale_socket_file() -> Result<()> {
        let dir = TempDir::new().unwrap();
        let path = socket_path(dir.path());
        // 造一个「上次崩溃留下的」空文件。
        std::fs::write(&path, b"")?;
        assert!(
            !probe_holder(dir.path()).await,
            "残留 socket 文件不应被判定为有持有者"
        );

        // 而且持有者应当能清理掉这个残留文件并成功 bind。
        let holder = transport_for_state_dir(dir.path())?;
        assert!(probe_holder(dir.path()).await);
        drop(holder);
        Ok(())
    }

    /// 已有持有者时，第二个进程不能抢监听：它只能作为观察者接入。
    #[tokio::test]
    async fn second_process_becomes_observer_instead_of_stealing() -> Result<()> {
        let dir = TempDir::new().unwrap();
        let holder = transport_for_state_dir(dir.path())?;

        let second = transport_for_state_dir(dir.path())?;
        // 不能用 expect_err：`Box<dyn SessionStream>` 没有 Debug。
        let err = match second.accept().await {
            Ok(_) => panic!("第二个进程不该能 accept"),
            Err(err) => err,
        };
        assert!(
            err.to_string().contains("不是 ipc 持有者"),
            "错误信息应说明角色，实际: {err}"
        );

        // 关键：持有者的监听没被抢走，观察者仍然能连上并通信。
        let mut observer = second.connect().await?;
        let mut holder_side = holder.accept().await?;
        observer.send(&ev(1, "hi")).await?;
        assert_eq!(holder_side.recv().await?, Some(ev(1, "hi")));
        Ok(())
    }

    /// 观察者断开：一端 drop 后另一端 `recv` 返回 `Ok(None)`，不能 panic。
    #[tokio::test]
    async fn peer_drop_yields_clean_eof() -> Result<()> {
        let dir = TempDir::new().unwrap();
        let holder = transport_for_state_dir(dir.path())?;
        let observer = transport_for_state_dir(dir.path())?;

        let mut observer_stream = observer.connect().await?;
        let mut holder_stream = holder.accept().await?;

        // 观测者先发一帧，再断开。
        observer_stream.send(&ev(1, "bye")).await?;
        assert_eq!(holder_stream.recv().await?, Some(ev(1, "bye")));
        drop(observer_stream);

        assert_eq!(
            holder_stream.recv().await?,
            None,
            "观察者断开后持有者应收到干净的 EOF"
        );
        // 重复 recv 也应当稳定返回 None，而不是 panic 或报错。
        assert_eq!(holder_stream.recv().await?, None);
        Ok(())
    }

    /// 持有者断开：观察者侧的 `recv` 同样返回 `Ok(None)`。
    #[tokio::test]
    async fn holder_drop_yields_clean_eof_for_observer() -> Result<()> {
        let dir = TempDir::new().unwrap();
        let holder = transport_for_state_dir(dir.path())?;
        let observer = transport_for_state_dir(dir.path())?;

        let mut observer_stream = observer.connect().await?;
        let mut holder_stream = holder.accept().await?;

        holder_stream.send(&ev(9, "last")).await?;
        assert_eq!(observer_stream.recv().await?, Some(ev(9, "last")));
        drop(holder_stream);

        assert_eq!(observer_stream.recv().await?, None);
        Ok(())
    }

    /// 持有者退出后端点应当被清理，下一个进程可以直接成为持有者。
    #[tokio::test]
    async fn holder_drop_releases_endpoint() -> Result<()> {
        let dir = TempDir::new().unwrap();
        {
            let holder = transport_for_state_dir(dir.path())?;
            assert!(probe_holder(dir.path()).await);
            drop(holder);
        }
        assert!(
            !probe_holder(dir.path()).await,
            "持有者退出后不该还能连上端点"
        );

        // 新进程能直接成为持有者。
        let next = transport_for_state_dir(dir.path())?;
        assert!(probe_holder(dir.path()).await);
        drop(next);
        Ok(())
    }

    /// 每个测试用自己的临时目录，端点不会互相干扰。
    #[tokio::test]
    async fn concurrent_state_dirs_do_not_interfere() -> Result<()> {
        let a = TempDir::new().unwrap();
        let b = TempDir::new().unwrap();

        let holder_a = transport_for_state_dir(a.path())?;
        let holder_b = transport_for_state_dir(b.path())?;
        assert_ne!(holder_a.endpoint(), holder_b.endpoint());

        // 两个持有者各自独立，连接不会串台。
        let obs_a = transport_for_state_dir(a.path())?;
        let mut obs_a = obs_a.connect().await?;
        let mut side_a = holder_a.accept().await?;
        obs_a.send(&ev(1, "only-a")).await?;
        assert_eq!(side_a.recv().await?, Some(ev(1, "only-a")));

        // b 上没有待处理连接，probe 为 true 但 accept 会阻塞——这里只验证端点隔离，
        // 不做 accept，避免测试挂住。
        assert!(probe_holder(b.path()).await);
        Ok(())
    }
}
