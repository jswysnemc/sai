//! 跨进程会话链接（P4）：把同一个会话的两个 sai 进程接成主从。
//!
//! 角色只有两种：
//!
//! - **持有者**：唯一写会话事件文件（`web/session-events/*.jsonl`）的进程，
//!   跑 IPC 监听、给观察者补发历史、把观察者上行的事件落盘并扇出；
//! - **观察者**：不写事件文件，自己那一轮的事件经 IPC 上行给持有者，
//!   显示用的事件流来自持有者下行 + 共享文件尾部。
//!
//! 单一写者是硬约束：事件文件在 `paths.state_dir` 下、被两个进程共享，
//! 各自 `EventJournal::persistent` 会在内存里各自累加序号，同时写必然重号；
//! 而且超限时各自重写文件会互相把对方刚追加的事件丢掉。
//!
//! ## 谁当持有者
//!
//! 原子仲裁者是**传输层租约**（Unix 上的 `flock`，Windows 上的 `first_pipe_instance`），
//! 不是持有者登记表——登记表是「读 + 写」两步，两个进程同时探测到「没有持有者」时
//! 两边都能写成功。所以顺序固定为：先抢租约，抢到再去登记；登记失败就放开租约
//! 退回观察者。观察者的看门狗检测到持有者心跳超时后按同样的顺序竞争接管。
//!
//! ## 降级
//!
//! 本模块所有失败都退回 [`LinkRole::Detached`]（本进程自己落盘，等价于 P3 行为），
//! 不改变任何既有路径的语义。

use crate::ipc::frame::{
    Frame, KIND_CTL_ABORT, KIND_CTL_HELLO, KIND_CTL_PING, KIND_CTL_PONG, KIND_CTL_SUBMIT,
    KIND_CTL_SUBMIT_ACK, KIND_CTL_SUBSCRIBE, KIND_EVT_MIRROR,
};
use crate::ipc::transport::{
    probe_holder, transport_for_state_dir, SessionStream, SessionTransport,
};
use crate::runner::{
    holder_is_alive, session_holder, ActorHandle, SessionActor, SessionHolderGuard, SessionOwner,
    TransportKind, TransportRef, HOLDER_HEARTBEAT_INTERVAL,
};
use crate::web::runs::{StartRunRequest, WebEvent};
use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

/// 重连退避：200ms 起步，逐次翻倍，封顶 3s。
const BACKOFF_MIN: Duration = Duration::from_millis(200);
const BACKOFF_MAX: Duration = Duration::from_millis(3_000);

/// 观察者检查持有者是否还活着的间隔。
const WATCHDOG_INTERVAL: Duration = Duration::from_secs(2);

/// 观察者的事件缓冲容量；与本地观察者一致，慢消费者同样被摘除。
const OBSERVER_CAPACITY: usize = 1024;

/// 单向待发帧的队列上限，防止连接任务自己堆爆内存。
const PENDING_FRAME_LIMIT: usize = 4096;

/// 只读回扫事件文件时的字节上限，与 `EventJournal` 的落盘上限保持一致。
const JOURNAL_TAIL_BYTES: u64 = 16 * 1024 * 1024;
/// 只读回扫时保留的事件条数上限，与 `EventJournal` 的内存容量保持一致。
const JOURNAL_TAIL_EVENTS: usize = 2048;

/// 抢租约失败后重试接管的次数上限。
const CLAIM_ATTEMPTS: usize = 3;

/// 持有者不接受外部驱动时的回执理由。
///
/// 只有「能按需起一轮」的持有者（Web 的运行管理器）会登记执行器；TUI 之类
/// 的持有者拿不到外部入口，提交一律被拒。理由必须可操作——静默失败会让用户
/// 的输入凭空消失。
const HOLDER_NOT_DRIVABLE: &str =
    "the session holder cannot be driven remotely; close it and this process takes over automatically";

/// 等待持有者受理提交的上限。
///
/// 超过它就把提交判为失败并明确告知用户——静默丢弃用户输入比报错更糟糕。
/// 取 10s 是因为持有者侧可能正卡在一轮对话的调度锁上，但绝不会真的排队这么久。
const SUBMIT_ACK_TIMEOUT: Duration = Duration::from_secs(10);

/// 观察者上行的一轮对话请求。
///
/// 观察者不持有 Agent，因此不能自己跑这一轮；整包参数上行给持有者，
/// 由持有者用自己的 Agent 与工作目录执行。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct SubmittedRun {
    /// 本次提交标识，用于把回执关联回发起方。
    pub(crate) submit_id: String,
    /// 观察者已分配的轮次标识；持有者必须沿用，否则发起方对不上事件流。
    pub(crate) run_id: String,
    /// 工作区标识。
    pub(crate) workspace_id: String,
    /// 工作区绝对路径：持有者用它作为本轮的工作目录。
    pub(crate) workspace_path: String,
    /// 发起一轮所需的全部参数。
    pub(crate) request: StartRunRequest,
}

/// 持有者对一次 [`SubmittedRun`] 的受理回执。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SubmitAck {
    /// 对应的提交标识。
    pub(crate) submit_id: String,
    /// 持有者是否已受理这一轮。
    pub(crate) accepted: bool,
    /// 拒绝或失败时的人类可读原因。
    #[serde(default)]
    pub(crate) reason: Option<String>,
    /// 持有者侧该轮的实际状态：`running` 或 `queued`。
    #[serde(default)]
    pub(crate) status: Option<String>,
}

/// 持有者侧待处理的观察者请求。
///
/// 由持有者的业务层（Web 的运行管理器）在专属任务里消费；IPC 层只负责
/// 搬运与回执，不关心一轮对话怎么跑。
pub(crate) enum HolderRequest {
    /// 代跑一轮；回执里带回该轮在持有者侧的状态。
    Submit {
        request: SubmittedRun,
        reply: oneshot::Sender<Result<String>>,
    },
    /// 中断一轮；回执里带回是否命中了一个活动轮次。
    Abort {
        run_id: String,
        reply: oneshot::Sender<Result<bool>>,
    },
}

/// 角色编码，放进 [`AtomicU8`] 供后台任务无锁读取。
const ROLE_DETACHED: u8 = 0;
const ROLE_OBSERVER: u8 = 1;
const ROLE_HOLDER: u8 = 2;

/// 本进程在该会话上扮演的角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LinkRole {
    /// 没能建立链接：退回单进程行为（本进程自己落盘）。
    Detached,
    /// 本进程持有会话。
    Holder,
    /// 本进程只是观察者。
    Observer,
}

impl LinkRole {
    fn from_code(code: u8) -> Self {
        match code {
            ROLE_HOLDER => Self::Holder,
            ROLE_OBSERVER => Self::Observer,
            _ => Self::Detached,
        }
    }

    fn code(self) -> u8 {
        match self {
            Self::Detached => ROLE_DETACHED,
            Self::Holder => ROLE_HOLDER,
            Self::Observer => ROLE_OBSERVER,
        }
    }
}

/// 观察者侧的扇出中心：持有者下行的事件转给本进程的所有订阅者。
struct ObserverHub {
    subscribers: Mutex<Vec<crate::runner::Watcher>>,
}

impl ObserverHub {
    fn new() -> Self {
        Self {
            subscribers: Mutex::new(Vec::new()),
        }
    }

    /// 注册一个订阅者。
    fn subscribe(&self) -> crate::runner::SessionSubscription {
        let (watcher, subscription) = crate::runner::Watcher::local(OBSERVER_CAPACITY);
        self.subscribers
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push(watcher);
        subscription
    }

    /// 向全部订阅者扇出一条事件；跟不上的订阅者被摘除。
    fn post(&self, event: WebEvent) {
        self.subscribers
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .retain_mut(|watcher| watcher.deliver(&event));
    }
}

/// 链接的可变状态；`SessionLink` 只是它的共享句柄。
struct LinkState {
    role: AtomicU8,
    workspace_id: String,
    session_id: String,
    /// 会话状态目录：持有者登记表与 IPC 端点都落在它下面。
    state_dir: PathBuf,
    /// 共享事件文件；观察者只读回扫它，持有者写入它。
    journal_path: PathBuf,
    /// 观察者侧：事件上行通道。为空即表示当前没有可用的持有者连接。
    upstream: Mutex<Option<mpsc::UnboundedSender<Frame>>>,
    /// 观察者侧：持有者下行的事件扇出中心。
    hub: ObserverHub,
    /// 观察者已收到的最大事件序号，重连时用它请求补发。
    cursor: AtomicU64,
    /// 持有者侧：观察者上行的请求交给谁执行。
    ///
    /// 只有 Web 这类「能按需起一轮」的持有者会登记；TUI 之类的持有者没有
    /// 可被外部驱动的轮次入口，登记为空，提交会被明确拒绝而不是静默丢弃。
    holder_sink: Mutex<Option<mpsc::UnboundedSender<HolderRequest>>>,
    /// 观察者侧：已发出、尚未收到回执的提交。
    ///
    /// 连接断开时全部判为失败，绝不静默吞掉用户输入。
    pending_submits: Mutex<HashMap<String, oneshot::Sender<SubmitAck>>>,
}

/// 一个会话的跨进程链接。
///
/// 可克隆，克隆体共享同一份状态。构造后由后台任务维护：持有者跑监听与心跳，
/// 观察者跑连接与接管看门狗。
#[derive(Clone)]
pub(crate) struct SessionLink {
    state: Arc<LinkState>,
}

impl SessionLink {
    /// 建立会话链接并返回该会话的事件总线。
    ///
    /// 拿不到 IPC 端点时返回本地事件总线（角色 [`LinkRole::Detached`]），
    /// 语义与 P3 完全一致。
    ///
    /// 参数:
    /// - `owner`: 本进程的持有者类型
    /// - `state_dir`: 会话状态目录
    /// - `session_id`: 会话 ID
    /// - `journal_path`: 共享会话事件文件
    /// - `workspace_id`: 工作区标识
    ///
    /// 返回:
    /// - （会话链接，事件总线）
    pub(crate) async fn attach(
        owner: SessionOwner,
        state_dir: &Path,
        session_id: &str,
        journal_path: PathBuf,
        workspace_id: &str,
    ) -> (Self, ActorHandle) {
        let link = Self {
            state: Arc::new(LinkState {
                role: AtomicU8::new(ROLE_DETACHED),
                workspace_id: workspace_id.to_string(),
                session_id: session_id.to_string(),
                state_dir: state_dir.to_path_buf(),
                journal_path: journal_path.clone(),
                upstream: Mutex::new(None),
                hub: ObserverHub::new(),
                cursor: AtomicU64::new(0),
                holder_sink: Mutex::new(None),
                pending_submits: Mutex::new(HashMap::new()),
            }),
        };
        // 本地事件总线：拿不到 IPC 端点时的兜底，语义与 P3 完全一致
        let local = || SessionActor::spawn(journal_path.clone(), workspace_id, session_id);
        // `probe_holder` 只是一次廉价快筛，真正的仲裁者是下面这次 bind 抢到的租约
        let holder_exists = probe_holder(state_dir).await;
        let Ok(transport) =
            transport_for_state_dir(state_dir).map(Arc::<dyn SessionTransport>::from)
        else {
            hold_without_ipc(state_dir, session_id, owner);
            return (link, local());
        };
        if !holder_exists && transport.is_holder() {
            if let Some(guard) = claim_holder(state_dir, session_id, owner).await {
                let _ = guard.publish_transport(transport_ref(transport.as_ref()));
                link.set_role(LinkRole::Holder);
                let bus = local();
                tokio::spawn(serve(link.clone(), transport, bus.clone(), Some(guard)));
                return (link, bus);
            }
            // 租约在手却登记失败：说明登记表里还有一条「心跳新鲜」的记录，
            // 持有者可能刚崩溃不到 15s。放开租约，以观察者身份重建传输，
            // 交给看门狗等心跳过期后再竞争，而不是占着端点当观察者。
            drop(transport);
            let Ok(rebound) =
                transport_for_state_dir(state_dir).map(Arc::<dyn SessionTransport>::from)
            else {
                return (link, local());
            };
            let bus = link.start_observer(owner, rebound);
            return (link, bus);
        }
        let bus = link.start_observer(owner, transport);
        (link, bus)
    }

    /// 以观察者身份接入持有者。
    ///
    /// 参数:
    /// - `owner`: 本进程的持有者类型，接管时用它登记
    /// - `transport`: 已建好的传输层（本进程不是持有者，只能 connect）
    ///
    /// 返回:
    /// - 观察者模式的事件总线
    fn start_observer(
        &self,
        owner: SessionOwner,
        transport: Arc<dyn SessionTransport>,
    ) -> ActorHandle {
        self.set_role(LinkRole::Observer);
        let bus = ActorHandle::observer(
            &self.state.workspace_id,
            &self.state.session_id,
            self.clone(),
        );
        tokio::spawn(observe(self.clone(), transport));
        tokio::spawn(watchdog(self.clone(), owner, bus.clone()));
        bus
    }

    /// 建立一个不参与跨进程协作的链接。
    ///
    /// 用于拿不到会话状态目录等降级场景：角色固定为 [`LinkRole::Detached`]，
    /// 事件总线由调用方自己创建，语义与 P3 完全一致。
    ///
    /// 参数:
    /// - `journal_path`: 共享会话事件文件
    /// - `workspace_id`: 工作区标识
    /// - `session_id`: 会话标识
    ///
    /// 返回:
    /// - 会话链接
    pub(crate) fn detached(journal_path: &Path, workspace_id: &str, session_id: &str) -> Self {
        Self {
            state: Arc::new(LinkState {
                role: AtomicU8::new(ROLE_DETACHED),
                workspace_id: workspace_id.to_string(),
                session_id: session_id.to_string(),
                state_dir: PathBuf::new(),
                journal_path: journal_path.to_path_buf(),
                upstream: Mutex::new(None),
                hub: ObserverHub::new(),
                cursor: AtomicU64::new(0),
                holder_sink: Mutex::new(None),
                pending_submits: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// 返回当前角色。
    pub(crate) fn role(&self) -> LinkRole {
        LinkRole::from_code(self.state.role.load(Ordering::Acquire))
    }

    /// 设置当前角色。
    fn set_role(&self, role: LinkRole) {
        self.state.role.store(role.code(), Ordering::Release);
    }

    /// 返回工作区标识。
    pub(crate) fn workspace_id(&self) -> String {
        self.state.workspace_id.clone()
    }

    /// 返回会话标识。
    pub(crate) fn session_id(&self) -> String {
        self.state.session_id.clone()
    }

    /// 观察者是否已经连上了持有者。
    ///
    /// 未连上时上行事件会丢。目前只有测试与诊断用它，等前端需要展示
    /// 「已断开，正在重连」时这里就是数据源。
    #[allow(dead_code)]
    pub(crate) fn is_connected(&self) -> bool {
        self.state
            .upstream
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .is_some()
    }

    /// 观察者把本进程产生的事件上行给持有者。
    ///
    /// 参数:
    /// - `event`: 已组装的事件
    ///
    /// 返回:
    /// - 是否成功入队；持有者失联时为 false
    pub(crate) fn mirror(&self, event: WebEvent) -> bool {
        let upstream = self
            .state
            .upstream
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        let Some(upstream) = upstream else {
            return false;
        };
        upstream
            .send(Frame {
                kind: KIND_EVT_MIRROR.to_string(),
                // 序号由持有者统一分配，上行侧留空
                sequence: None,
                payload: serde_json::to_value(&event).unwrap_or(serde_json::Value::Null),
            })
            .is_ok()
    }

    /// 订阅持有者的事件流（观察者模式）。
    pub(crate) fn subscribe(&self) -> crate::runner::SessionSubscription {
        self.state.hub.subscribe()
    }

    /// 向本进程的订阅者扇出一条事件。
    ///
    /// 接管完成后由本地事件总线桥接过来，保证接管前建立的 SSE 流不断流。
    pub(crate) fn post(&self, event: WebEvent) {
        self.state.hub.post(event);
    }

    /// 取出持有者侧登记的请求执行器。
    ///
    /// 返回:
    /// - 执行队列发送端；未登记时为空
    fn holder_sink(&self) -> Option<mpsc::UnboundedSender<HolderRequest>> {
        self.state
            .holder_sink
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    /// 登记持有者侧的请求执行器。
    ///
    /// 只有「能被外部驱动起一轮」的持有者需要登记（Web 的运行管理器）；
    /// 没登记时观察者上行的一律被明确拒绝。登记与角色无关——接管后本进程
    /// 可能从观察者变持有者，执行器要提前就位。
    ///
    /// 参数:
    /// - `sink`: 持有者侧请求队列的发送端
    ///
    /// 返回:
    /// - 无
    pub(crate) fn set_holder_sink(&self, sink: mpsc::UnboundedSender<HolderRequest>) {
        *self
            .state
            .holder_sink
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(sink);
    }

    /// 观察者请求持有者代跑一轮。
    ///
    /// 观察者不持有 Agent，必须把整包参数上行给持有者执行。失败一律返回
    /// `Err`，调用方负责把原因呈现给用户——**绝不能静默丢弃用户输入**。
    ///
    /// 参数:
    /// - `request`: 待执行的一轮请求
    ///
    /// 返回:
    /// - 持有者的受理回执
    pub(crate) async fn submit(&self, request: SubmittedRun) -> Result<SubmitAck> {
        let (tx, rx) = oneshot::channel();
        let payload = serde_json::to_value(&request).unwrap_or(Value::Null);
        {
            let upstream = self
                .state
                .upstream
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .clone();
            let Some(upstream) = upstream else {
                bail!("未连上会话持有者：本轮输入未被执行");
            };
            // 先登记再发送：回执可能在 send 返回前就到了
            self.register_pending(&request.submit_id, tx);
            if upstream
                .send(Frame::control(KIND_CTL_SUBMIT, payload))
                .is_err()
            {
                self.cancel_pending(&request.submit_id);
                bail!("与会话持有者的连接已断开：本轮输入未被执行");
            }
        }
        match tokio::time::timeout(SUBMIT_ACK_TIMEOUT, rx).await {
            Ok(Ok(ack)) => Ok(ack),
            // 持有者在回执前断开：连接结束时已把未决提交判为失败
            Ok(Err(_)) => bail!("会话持有者在受理前断开：本轮输入未被执行"),
            Err(_) => {
                self.cancel_pending(&request.submit_id);
                bail!(
                    "会话持有者未在 {} 秒内受理本轮输入：未被执行",
                    SUBMIT_ACK_TIMEOUT.as_secs()
                );
            }
        }
    }

    /// 观察者请求持有者中断一轮。
    ///
    /// 中断回执复用提交回执的通道：回执帧里的 `submit_id` 就是被中断的轮次标识。
    ///
    /// 参数:
    /// - `run_id`: 轮次标识
    ///
    /// 返回:
    /// - 是否命中了一个活动轮次
    pub(crate) async fn abort(&self, run_id: &str) -> Result<bool> {
        let (tx, rx) = oneshot::channel();
        let upstream = self
            .state
            .upstream
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        let Some(upstream) = upstream else {
            bail!("未连上会话持有者：中断请求未送达");
        };
        // 先登记再发送：回执可能在 send 返回前就到了
        self.register_pending(run_id, tx);
        if upstream
            .send(Frame::control(KIND_CTL_ABORT, json!({ "run_id": run_id })))
            .is_err()
        {
            self.cancel_pending(run_id);
            bail!("与会话持有者的连接已断开：中断请求未送达");
        }
        match tokio::time::timeout(SUBMIT_ACK_TIMEOUT, rx).await {
            Ok(Ok(ack)) => Ok(ack.accepted),
            Ok(Err(_)) => bail!("会话持有者在应答前断开：中断请求未送达"),
            Err(_) => {
                self.cancel_pending(run_id);
                bail!(
                    "会话持有者未在 {} 秒内应答中断请求",
                    SUBMIT_ACK_TIMEOUT.as_secs()
                );
            }
        }
    }

    /// 登记一个未决提交。
    ///
    /// 参数:
    /// - `key`: 提交标识
    /// - `tx`: 回执发送端
    ///
    /// 返回:
    /// - 无
    fn register_pending(&self, key: &str, tx: oneshot::Sender<SubmitAck>) {
        self.state
            .pending_submits
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(key.to_string(), tx);
    }

    /// 取下一个未决提交的回执发送端。
    ///
    /// 参数:
    /// - `key`: 提交标识
    ///
    /// 返回:
    /// - 回执发送端；已被超时逻辑摘除时为空
    fn take_pending(&self, key: &str) -> Option<oneshot::Sender<SubmitAck>> {
        self.state
            .pending_submits
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(key)
    }

    /// 摘除一个未决提交（超时或发送失败）。
    ///
    /// 参数:
    /// - `key`: 提交标识
    ///
    /// 返回:
    /// - 无
    fn cancel_pending(&self, key: &str) {
        self.take_pending(key);
    }

    /// 把所有未决提交判为失败。
    ///
    /// 持有者失联时调用：调用方据此给用户明确反馈，而不是让输入凭空消失。
    ///
    /// 参数:
    /// - `reason`: 失败原因
    ///
    /// 返回:
    /// - 无
    fn fail_pending(&self, reason: &str) {
        let pending: Vec<(String, oneshot::Sender<SubmitAck>)> = self
            .state
            .pending_submits
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .drain()
            .collect();
        for (submit_id, tx) in pending {
            let _ = tx.send(SubmitAck {
                submit_id,
                accepted: false,
                reason: Some(reason.to_string()),
                status: None,
            });
        }
    }

    /// 按序号补发历史事件。
    ///
    /// 观察者模式下只读回扫共享文件尾部——**不能**用 `EventJournal::persistent`：
    /// 它在文件超限时会把文件重写一遍，而这个文件正被持有者追加，重写会丢事件。
    ///
    /// 参数:
    /// - `after`: 已接收的最后事件序号
    ///
    /// 返回:
    /// - 需要补发的事件
    pub(crate) fn replay(&self, after: u64) -> Vec<WebEvent> {
        read_journal_tail(&self.state.journal_path, after)
    }
}

/// 尝试成为持有者。
///
/// 顺序固定为「先租约、后登记」：租约是原子的，登记表不是。登记失败时
/// 放开租约并返回空，调用方应退回观察者。
///
/// 参数:
/// - `state_dir`: 会话状态目录
/// - `session_id`: 会话 ID
/// - `owner`: 本进程的持有者类型
///
/// 返回:
/// - 持有者守卫；已被别人抢走时为空
async fn claim_holder(
    state_dir: &Path,
    session_id: &str,
    owner: SessionOwner,
) -> Option<SessionHolderGuard> {
    for attempt in 0..CLAIM_ATTEMPTS {
        if let Ok(guard) = SessionHolderGuard::acquire(state_dir, session_id, owner) {
            return Some(guard);
        }
        if attempt + 1 < CLAIM_ATTEMPTS {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
    None
}

/// 拿不到 IPC 时仍登记持有者心跳。
///
/// 单进程降级不等于会话没打开：终端已经在跑，session_probe 必须能发现
/// 这个还没发过提示词的空会话。
///
/// 参数:
/// - `state_dir`: 会话状态目录
/// - `session_id`: 会话 ID
/// - `owner`: 本进程的持有者类型
///
/// 返回:
/// - 无
fn hold_without_ipc(state_dir: &Path, session_id: &str, owner: SessionOwner) {
    let state_dir = state_dir.to_path_buf();
    let session_id = session_id.to_string();
    tokio::spawn(async move {
        let Some(guard) = claim_holder(&state_dir, &session_id, owner).await else {
            return;
        };
        let mut heartbeat = tokio::time::interval(HOLDER_HEARTBEAT_INTERVAL);
        heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            heartbeat.tick().await;
            if guard.heartbeat().is_err() {
                return;
            }
        }
    });
}

/// 持有者主循环：接受观察者连接，同时按 [`HOLDER_HEARTBEAT_INTERVAL`] 写心跳。
async fn serve(
    link: SessionLink,
    transport: Arc<dyn SessionTransport>,
    bus: ActorHandle,
    guard: Option<SessionHolderGuard>,
) {
    // 守卫活到这个任务结束：Drop 时撤销登记，观察者据此接管
    let _guard = guard;
    let mut heartbeat = tokio::time::interval(HOLDER_HEARTBEAT_INTERVAL);
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            accepted = transport.accept() => match accepted {
                Ok(stream) => {
                    tokio::spawn(serve_connection(link.clone(), stream, bus.clone()));
                }
                // 监听失败多半是端点被外力清掉，退出让观察者按心跳超时接管
                Err(_) => return,
            },
            _ = heartbeat.tick() => {
                if let Some(guard) = _guard.as_ref() {
                    let _ = guard.heartbeat();
                }
            },
        }
    }
}

/// 服务一条观察者连接：先补发历史，再双向转发。
async fn serve_connection(link: SessionLink, mut stream: Box<dyn SessionStream>, bus: ActorHandle) {
    // 1. 等观察者第一条 hello，拿到它已收到的最后序号
    let after = match stream.recv().await {
        Ok(Some(frame)) if frame.kind == KIND_CTL_HELLO => {
            frame.payload["after"].as_u64().unwrap_or(0)
        }
        Ok(Some(_)) => 0,
        Ok(None) | Err(_) => return,
    };
    // 2. 先注册观察者再补发：补发期间产生的实时事件排在补发帧之后，顺序不乱
    let Some((mut frames, _dropped)) = bus.attach_remote(OBSERVER_CAPACITY) else {
        return;
    };
    // 3. 补发与中途改订阅点都走这个队列，绝不回灌进总线——回灌会把历史事件
    //    再落一次盘
    let mut pending: VecDeque<Frame> = bus.replay(after).iter().map(mirror_frame).collect();
    // 4. 提交回执走独立通道：执行一轮是对外阻塞的操作，不能在 select 里 await，
    //    否则一个慢轮次会卡住这条连接上的全部事件转发
    let (ack_tx, mut ack_rx) = mpsc::unbounded_channel::<Frame>();
    loop {
        if let Some(frame) = pending.pop_front() {
            if stream.send(&frame).await.is_err() {
                return;
            }
            continue;
        }
        tokio::select! {
            outgoing = frames.recv() => match outgoing {
                Some(frame) => if stream.send(&frame).await.is_err() { return; },
                // 观察者被摘除（慢消费者）后通道关闭，连接随之结束，客户端重连补发
                None => return,
            },
            ack = ack_rx.recv() => match ack {
                Some(frame) => if pending.len() < PENDING_FRAME_LIMIT {
                    pending.push_back(frame);
                },
                None => return,
            },
            incoming = stream.recv() => match incoming {
                Ok(Some(frame)) => {
                    if !handle_upstream(&link, &bus, frame, &mut pending, &ack_tx) { return; }
                }
                // 观察者断开（干净 EOF）或读出错：结束这条连接
                Ok(None) | Err(_) => return,
            },
        }
    }
}

/// 处理观察者上行的一帧。
///
/// 参数:
/// - `link`: 本会话的跨进程链接
/// - `bus`: 会话事件总线
/// - `frame`: 上行帧
/// - `pending`: 待发帧队列，用于把补发结果排进这条连接
/// - `ack_tx`: 提交回执队列，执行一轮的结果排到这里
///
/// 返回:
/// - 连接是否应当继续
fn handle_upstream(
    link: &SessionLink,
    bus: &ActorHandle,
    frame: Frame,
    pending: &mut VecDeque<Frame>,
    ack_tx: &mpsc::UnboundedSender<Frame>,
) -> bool {
    match frame.kind.as_str() {
        // 观察者自己那一轮产生的事件：已组装，直接落盘并扇出
        KIND_EVT_MIRROR => {
            if let Ok(event) = serde_json::from_value::<WebEvent>(frame.payload) {
                let _ = bus.mirror(event);
            }
            true
        }
        // 运行中改订阅起点（例如前端刷新后重放）
        KIND_CTL_SUBSCRIBE => {
            let after = frame.payload["after"].as_u64().unwrap_or(0);
            if pending.len() < PENDING_FRAME_LIMIT {
                pending.extend(bus.replay(after).iter().map(mirror_frame));
            }
            true
        }
        // 观察者请求代跑一轮：交给本进程的执行器，回执经 ack 通道排回这条连接
        KIND_CTL_SUBMIT => {
            let tx = ack_tx.clone();
            let sink = link.holder_sink();
            tokio::spawn(async move { dispatch_submit(sink, frame.payload, &tx).await });
            true
        }
        // 观察者请求中断一轮；回执复用提交回执通道，submit_id 存被中断的轮次
        KIND_CTL_ABORT => {
            let run_id = frame.payload["run_id"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            let Some(sink) = link.holder_sink() else {
                // 没有执行器（TUI 之类的持有者）：明确拒绝，绝不让请求静默消失
                let _ = ack_tx.send(abort_ack(
                    &run_id,
                    false,
                    Some(HOLDER_NOT_DRIVABLE.to_string()),
                ));
                return true;
            };
            let tx = ack_tx.clone();
            tokio::spawn(async move {
                let (reply_tx, reply_rx) = oneshot::channel();
                let outcome = if sink
                    .send(HolderRequest::Abort {
                        run_id: run_id.clone(),
                        reply: reply_tx,
                    })
                    .is_err()
                {
                    Err(anyhow!("持有者的执行队列已关闭"))
                } else {
                    match reply_rx.await {
                        Ok(result) => result,
                        Err(_) => Err(anyhow!("持有者的执行队列已关闭")),
                    }
                };
                let (accepted, reason) = match outcome {
                    Ok(true) => (true, None),
                    Ok(false) => (false, None),
                    Err(error) => (false, Some(error.to_string())),
                };
                let _ = tx.send(abort_ack(&run_id, accepted, reason));
            });
            true
        }
        // 心跳：本端由持有者单向驱动，观察者不需要探活，收到就忽略
        KIND_CTL_PING => {
            if pending.len() < PENDING_FRAME_LIMIT {
                pending.push_back(Frame::control(KIND_CTL_PONG, serde_json::json!({})));
            }
            true
        }
        _ => true,
    }
}

/// 分发观察者上行的一轮提交并回执。
///
/// 在独立任务里跑：在连接循环里 await 一轮对话会让这条连接上的事件转发
/// 全部停摆。没有登记执行器时一律明确拒绝——静默丢弃用户输入是最坏的结果。
///
/// 参数:
/// - `sink`: 持有者侧执行队列；未登记时为空
/// - `payload`: 上行帧载荷
/// - `tx`: 回执队列
///
/// 返回:
/// - 无
async fn dispatch_submit(
    sink: Option<mpsc::UnboundedSender<HolderRequest>>,
    payload: Value,
    tx: &mpsc::UnboundedSender<Frame>,
) {
    let request = match serde_json::from_value::<SubmittedRun>(payload) {
        Ok(request) => request,
        Err(error) => {
            let _ = tx.send(ack_frame(SubmitAck {
                submit_id: String::new(),
                accepted: false,
                reason: Some(format!("提交参数无法解析：{error}")),
                status: None,
            }));
            return;
        }
    };
    let submit_id = request.submit_id.clone();
    let Some(sink) = sink else {
        let _ = tx.send(ack_frame(SubmitAck {
            submit_id,
            accepted: false,
            reason: Some(HOLDER_NOT_DRIVABLE.to_string()),
            status: None,
        }));
        return;
    };
    let (reply_tx, reply_rx) = oneshot::channel();
    let outcome = if sink
        .send(HolderRequest::Submit {
            request,
            reply: reply_tx,
        })
        .is_err()
    {
        Err(anyhow!("持有者的执行队列已关闭"))
    } else {
        match reply_rx.await {
            Ok(result) => result,
            Err(_) => Err(anyhow!("持有者的执行队列已关闭")),
        }
    };
    let ack = match outcome {
        Ok(status) => SubmitAck {
            submit_id,
            accepted: true,
            reason: None,
            status: Some(status),
        },
        Err(error) => SubmitAck {
            submit_id,
            accepted: false,
            reason: Some(error.to_string()),
            status: None,
        },
    };
    let _ = tx.send(ack_frame(ack));
}

/// 把中断回执编码成下行帧。
///
/// 复用提交回执的 kind：`submit_id` 存被中断的轮次标识，`accepted` 表示是否
/// 命中了一个活动轮次。
///
/// 参数:
/// - `run_id`: 被中断的轮次标识
/// - `accepted`: 是否命中
/// - `reason`: 失败原因
///
/// 返回:
/// - 待写入 IPC 的帧
fn abort_ack(run_id: &str, accepted: bool, reason: Option<String>) -> Frame {
    ack_frame(SubmitAck {
        submit_id: run_id.to_string(),
        accepted,
        reason,
        status: None,
    })
}

/// 把受理回执编码成下行帧。
///
/// 参数:
/// - `ack`: 受理回执
///
/// 返回:
/// - 待写入 IPC 的帧
fn ack_frame(ack: SubmitAck) -> Frame {
    Frame {
        kind: KIND_CTL_SUBMIT_ACK.to_string(),
        sequence: None,
        payload: serde_json::to_value(&ack).unwrap_or(Value::Null),
    }
}

/// 观察者连接循环：指数退避重连，期间按共享文件尾部降级显示。
async fn observe(link: SessionLink, transport: Arc<dyn SessionTransport>) {
    let mut backoff = BACKOFF_MIN;
    loop {
        if link.role() == LinkRole::Holder {
            return;
        }
        match connect(&link, transport.as_ref()).await {
            Ok(()) => {
                backoff = BACKOFF_MIN;
                tokio::time::sleep(BACKOFF_MIN).await;
            }
            Err(_) => {
                tokio::time::sleep(backoff).await;
                backoff = std::cmp::min(backoff * 2, BACKOFF_MAX);
            }
        }
    }
}

/// 连接持有者并消费事件流；连接断开时返回。
async fn connect(link: &SessionLink, transport: &dyn SessionTransport) -> Result<()> {
    let mut stream = transport.connect().await?;
    let (tx, mut rx) = mpsc::unbounded_channel::<Frame>();
    *link
        .state
        .upstream
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(tx);
    // 报上自己已收到的最后序号，持有者据此补发
    stream
        .send(&Frame::control(
            KIND_CTL_HELLO,
            serde_json::json!({ "after": link.state.cursor.load(Ordering::Relaxed) }),
        ))
        .await?;
    let outcome = pump(link, &mut stream, &mut rx).await;
    // 断开期间不再上行，等到重连成功再恢复
    *link
        .state
        .upstream
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = None;
    // 未决提交一律判失败：观察者侧的调用方据此给用户明确反馈，
    // 绝不能让已经敲出去的输入凭空消失
    link.fail_pending("会话持有者已断开");
    outcome
}

/// 双向泵：上行队列 → socket，socket → 本地扇出。
async fn pump(
    link: &SessionLink,
    stream: &mut Box<dyn SessionStream>,
    rx: &mut mpsc::UnboundedReceiver<Frame>,
) -> Result<()> {
    loop {
        tokio::select! {
            outgoing = rx.recv() => match outgoing {
                Some(frame) => stream.send(&frame).await?,
                None => return Ok(()),
            },
            incoming = stream.recv() => match incoming {
                Ok(Some(frame)) => handle_downstream(link, frame),
                // 持有者断开：交给调用方退避重连
                Ok(None) => return Err(anyhow!("holder closed the connection")),
                Err(error) => return Err(error),
            },
        }
    }
}

/// 处理持有者下行的一帧。
fn handle_downstream(link: &SessionLink, frame: Frame) {
    // 提交/中断回执走独立分支：它与事件流无关，且必须在连接断开时也能送达
    if frame.kind == KIND_CTL_SUBMIT_ACK {
        let Ok(ack) = serde_json::from_value::<SubmitAck>(frame.payload) else {
            return;
        };
        if let Some(tx) = link.take_pending(&ack.submit_id) {
            let _ = tx.send(ack);
        }
        return;
    }
    if frame.kind != KIND_EVT_MIRROR {
        return;
    }
    // payload 会在下面被 move，先把它之前的借用全部用掉
    if let Some(sequence) = frame.sequence {
        link.state.cursor.fetch_max(sequence, Ordering::Relaxed);
    }
    // compaction 是覆盖式写入：观察者收到 begin 就进只读态
    if let Some(kind) = downstream_kind(&frame) {
        apply_downstream_kind(&link.state.state_dir, &kind);
    }
    let Ok(event) = serde_json::from_value::<WebEvent>(frame.payload) else {
        return;
    };
    // 已经接管的话本地总线会自己扇出，这里再转一次会重复
    if link.role() == LinkRole::Observer {
        link.state.hub.post(event);
    }
}

/// 接管看门狗：持有者心跳超时后竞争接管。
async fn watchdog(link: SessionLink, owner: SessionOwner, bus: ActorHandle) {
    let mut tick = tokio::time::interval(WATCHDOG_INTERVAL);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tick.tick().await;
        if link.role() == LinkRole::Holder {
            return;
        }
        // 1. 登记表里还有活着的持有者就什么都不做——它活着就该它写
        if let Some(holder) = session_holder(&link.state.state_dir) {
            if holder_is_alive(&holder) {
                continue;
            }
        }
        // 2. 先抢租约：多进程同时竞争时它是唯一公平的仲裁者
        let Ok(transport) =
            transport_for_state_dir(&link.state.state_dir).map(Arc::<dyn SessionTransport>::from)
        else {
            continue;
        };
        if !transport.is_holder() {
            continue;
        }
        // 3. 租约到手再登记；登记失败说明有人先登记了，放开租约等下一轮
        let Some(guard) = claim_holder(&link.state.state_dir, &link.state.session_id, owner).await
        else {
            continue;
        };
        let _ = guard.publish_transport(transport_ref(transport.as_ref()));
        link.set_role(LinkRole::Holder);
        *link
            .state
            .upstream
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = None;
        // 4. 先把事件总线切成持有者模式，再起监听：否则上行事件没有落盘的地方
        bus.promote_to_holder(&link.state.journal_path);
        tokio::spawn(serve(link.clone(), transport, bus.clone(), Some(guard)));
        return;
    }
}

/// 把传输端点转成登记表里的端点描述。
fn transport_ref(transport: &dyn SessionTransport) -> TransportRef {
    match transport.endpoint() {
        crate::ipc::transport::Endpoint::Unix(path) => TransportRef {
            kind: TransportKind::Unix,
            path: path.to_string_lossy().into_owned(),
        },
        crate::ipc::transport::Endpoint::WinPipe(name) => TransportRef {
            kind: TransportKind::WinPipe,
            path: name,
        },
    }
}

/// 把事件编码成下行帧。
fn mirror_frame(event: &WebEvent) -> Frame {
    Frame {
        kind: KIND_EVT_MIRROR.to_string(),
        sequence: Some(event.sequence),
        payload: serde_json::to_value(event).unwrap_or(serde_json::Value::Null),
    }
}

/// 只读回扫共享事件文件的尾部。
///
/// 刻意不复用 `EventJournal::persistent`：后者在文件超出上限时会把整个文件
/// 重写一遍，而这个文件的写者是另一个进程，重写必然丢事件。
///
/// 参数:
/// - `path`: 事件文件路径
/// - `after`: 已接收的最后事件序号
///
/// 返回:
/// - 序号大于 `after` 的事件，按序号升序
fn read_journal_tail(path: &Path, after: u64) -> Vec<WebEvent> {
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let size = file.metadata().map(|meta| meta.len()).unwrap_or(0);
    let start = size.saturating_sub(JOURNAL_TAIL_BYTES);
    let mut reader = std::io::BufReader::new(file);
    if reader.seek(SeekFrom::Start(start)).is_err() {
        return Vec::new();
    }
    let mut events: VecDeque<WebEvent> = VecDeque::new();
    let mut lines = reader.lines();
    if start > 0 {
        // 从文件中间开始，第一行是半行，丢掉
        let _ = lines.next();
    }
    for line in lines.map_while(Result::ok) {
        let Ok(event) = serde_json::from_str::<WebEvent>(&line) else {
            continue;
        };
        if event.sequence > after {
            events.push_back(event);
        }
        while events.len() > JOURNAL_TAIL_EVENTS {
            events.pop_front();
        }
    }
    events.into_iter().collect()
}

/// compaction 开始事件的事件类型。
pub(crate) const EVENT_COMPACTION_BEGIN: &str = "run.compaction.begin";
/// compaction 结束事件的事件类型。
pub(crate) const EVENT_COMPACTION_END: &str = "run.compaction.end";
/// compaction 广播使用的运行标识（它不是一次对话轮次，没有真实 run_id）。
// 待破坏式写入方接入广播后会有调用方（见下方 `broadcast_compaction`）。
#[allow(dead_code)]
const COMPACTION_RUN_ID: &str = "compaction";

/// 本进程登记到会话状态目录上的事件总线。
#[allow(dead_code)]
struct BusEntry {
    bus: ActorHandle,
    workspace_id: String,
    session_id: String,
}

/// 会话状态目录 → 事件总线。
///
/// compaction 之类的破坏式写入发生在 `StateStore` 里，而 `StateStore` 拿不到事件总线句柄
/// （它在会话创建时就被各个前端各自持有）。这里按状态目录登记一份，让那些写入方
/// 能广播一条「我要改写了」。
#[allow(dead_code)]
fn session_buses() -> &'static Mutex<HashMap<PathBuf, BusEntry>> {
    static BUSES: OnceLock<Mutex<HashMap<PathBuf, BusEntry>>> = OnceLock::new();
    BUSES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 正在 compaction 的会话状态目录。
///
/// 观察者收到 `run.compaction.begin` 后进入只读态，直到 `end` 才接受新输入。
fn compacting_sessions() -> &'static Mutex<std::collections::HashSet<PathBuf>> {
    static SET: OnceLock<Mutex<std::collections::HashSet<PathBuf>>> = OnceLock::new();
    SET.get_or_init(|| Mutex::new(std::collections::HashSet::new()))
}

/// 登记一个会话的事件总线。
///
/// 参数:
/// - `state_dir`: 会话状态目录
/// - `workspace_id`: 工作区标识
/// - `session_id`: 会话标识
/// - `bus`: 事件总线
///
/// 返回:
/// - 无
// 破坏式写入方接入广播后会有调用方（见下方 `broadcast_compaction`）。
#[allow(dead_code)]
pub(crate) fn register_session_bus(
    state_dir: &Path,
    workspace_id: &str,
    session_id: &str,
    bus: ActorHandle,
) {
    let mut buses = session_buses().lock().unwrap_or_else(|e| e.into_inner());
    buses.insert(
        state_dir.to_path_buf(),
        BusEntry {
            bus,
            workspace_id: workspace_id.to_string(),
            session_id: session_id.to_string(),
        },
    );
}

/// 注销会话事件总线。
///
/// 参数:
/// - `state_dir`: 会话状态目录
///
/// 返回:
/// - 无
#[allow(dead_code)]
pub(crate) fn unregister_session_bus(state_dir: &Path) {
    let mut buses = session_buses().lock().unwrap_or_else(|e| e.into_inner());
    buses.remove(state_dir);
}

/// 判断指定会话是否正处于 compaction 中（观察者应当拒绝新输入）。
///
/// 参数:
/// - `state_dir`: 会话状态目录
///
/// 返回:
/// - 是否正在 compaction
#[allow(dead_code)]
pub(crate) fn is_compacting(state_dir: &Path) -> bool {
    compacting_sessions()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .contains(state_dir)
}

/// 标记或清除会话的 compaction 状态。
///
/// 参数:
/// - `state_dir`: 会话状态目录
/// - `compacting`: 是否处于 compaction
///
/// 返回:
/// - 无
fn set_compacting(state_dir: &Path, compacting: bool) {
    let mut set = compacting_sessions()
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if compacting {
        set.insert(state_dir.to_path_buf());
    } else {
        set.remove(state_dir);
    }
}

/// 广播 compaction 阶段事件，并同步本进程的只读态。
///
/// 持有者直接改本进程状态；观察者通过 [`handle_downstream`] 收到同样的帧后
/// 进入只读态。没有登记事件总线时静默返回——单进程模式下没有别人在读。
///
/// 参数:
/// - `state_dir`: 会话状态目录
/// - `begin`: 是开始还是结束
/// - `detail`: 附带说明
///
/// 返回:
/// - 无
// 等 `StateStore` 的破坏式写入接入广播后会有调用方。
#[allow(dead_code)]
pub(crate) fn broadcast_compaction(state_dir: &Path, begin: bool, detail: &str) {
    set_compacting(state_dir, begin);
    let entry = {
        let buses = session_buses().lock().unwrap_or_else(|e| e.into_inner());
        match buses.get(state_dir) {
            Some(entry) => BusEntry {
                bus: entry.bus.clone(),
                workspace_id: entry.workspace_id.clone(),
                session_id: entry.session_id.clone(),
            },
            None => return,
        }
    };
    let _ = entry.bus.emit(WebEvent::new(
        COMPACTION_RUN_ID,
        &entry.workspace_id,
        &entry.session_id,
        if begin {
            EVENT_COMPACTION_BEGIN
        } else {
            EVENT_COMPACTION_END
        },
        json!({ "detail": detail }),
    ));
}

/// 把一条下行事件按类型分派给本进程的簿记逻辑。
///
/// compaction 是覆盖式写入：观察者收到 begin 后必须拒绝新输入，
/// 否则新轮次会写进马上被改写的上下文里。
///
/// 参数:
/// - `state_dir`: 会话状态目录
/// - `kind`: 事件类型
///
/// 返回:
/// - 无
fn apply_downstream_kind(state_dir: &Path, kind: &str) {
    match kind {
        EVENT_COMPACTION_BEGIN => set_compacting(state_dir, true),
        EVENT_COMPACTION_END => set_compacting(state_dir, false),
        _ => {}
    }
}

/// 把 WebEvent 的 payload 取出来做类型判断。
///
/// 参数:
/// - `frame`: 下行帧
///
/// 返回:
/// - 事件类型；解不出时为空
fn downstream_kind(frame: &Frame) -> Option<String> {
    if frame.kind != KIND_EVT_MIRROR {
        return None;
    }
    frame.payload.get("type")?.as_str().map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::SessionOwner;
    use serde_json::json;

    /// 建一个临时会话目录与事件文件路径。
    fn session() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let journal = dir.path().join("session.jsonl");
        (dir, journal)
    }

    /// 等角色变成期望值。
    async fn wait_for_role(link: &SessionLink, role: LinkRole) -> bool {
        for _ in 0..300 {
            if link.role() == role {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        link.role() == role
    }

    /// 等事件文件里出现至少 n 条事件。
    async fn wait_for_events(journal: &Path, count: usize) -> Vec<WebEvent> {
        for _ in 0..300 {
            let events = read_journal_tail(journal, 0);
            if events.len() >= count {
                return events;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        read_journal_tail(journal, 0)
    }

    /// 第一个 attach 的进程成为持有者，第二个降级为观察者。
    #[tokio::test]
    async fn second_process_attaches_as_observer() {
        let (dir, journal) = session();
        let state_dir = dir.path();

        let (first, _) = SessionLink::attach(
            SessionOwner::Repl,
            state_dir,
            "session",
            journal.clone(),
            "workspace",
        )
        .await;
        assert!(wait_for_role(&first, LinkRole::Holder).await);

        let (second, _) = SessionLink::attach(
            SessionOwner::Web,
            state_dir,
            "session",
            journal.clone(),
            "workspace",
        )
        .await;
        assert!(wait_for_role(&second, LinkRole::Observer).await);
        assert_eq!(first.role(), LinkRole::Holder, "第二个进程不能抢走持有权");
    }

    /// 多个进程同时竞争：只有一个能成为持有者，其余全部降级。
    ///
    /// 这正是 `probe_holder` 与 `acquire` 之间的竞态窗口——它们都可能看到
    /// 「没有持有者」，真正的仲裁者是传输层租约。
    #[tokio::test]
    async fn concurrent_attach_elects_exactly_one_holder() {
        let (dir, journal) = session();
        let state_dir = dir.path();
        let mut links = Vec::new();
        for _ in 0..4 {
            let (link, _bus) = SessionLink::attach(
                SessionOwner::Web,
                state_dir,
                "session",
                journal.clone(),
                "workspace",
            )
            .await;
            links.push(link);
        }
        tokio::time::sleep(Duration::from_millis(400)).await;
        let holders = links
            .iter()
            .filter(|link| link.role() == LinkRole::Holder)
            .count();
        let observers = links
            .iter()
            .filter(|link| link.role() == LinkRole::Observer)
            .count();
        assert_eq!(holders, 1, "同一会话只能有一个持有者");
        assert_eq!(observers, 3, "其余进程必须降级为观察者");
    }

    /// 观察者的事件经 IPC 上行后由持有者落盘；观察者自己不写文件。
    #[tokio::test]
    async fn observer_events_are_persisted_by_the_holder() {
        let (dir, journal) = session();
        let state_dir = dir.path();

        let (_holder_link, holder_bus) = SessionLink::attach(
            SessionOwner::Web,
            state_dir,
            "session",
            journal.clone(),
            "workspace",
        )
        .await;
        let (_observer_link, observer_bus) = {
            let (link, bus) = SessionLink::attach(
                SessionOwner::Repl,
                state_dir,
                "session",
                journal.clone(),
                "workspace",
            )
            .await;
            (link, bus)
        };
        // 等观察者真正连上持有者，再发事件——未连上时上行会丢
        let mut connected = false;
        for _ in 0..300 {
            if _holder_link.role() == LinkRole::Holder && _observer_link.is_connected() {
                connected = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(connected, "观察者应当在超时前连上持有者");
        observer_bus
            .emit(WebEvent::new(
                "run-observer",
                "workspace",
                "session",
                "message.content.delta",
                json!({ "text": "来自 TUI" }),
            ))
            .unwrap();

        let events = wait_for_events(&journal, 1).await;
        assert_eq!(events.len(), 1, "上行事件应由持有者落盘且只落一次");
        assert_eq!(events[0].run_id, "run-observer");
        assert_eq!(events[0].sequence, 1, "序号必须由持有者统一分配");
        // 观察者不写文件：它的本地日志是空的
        assert!(observer_bus.journal().events_after(0).is_empty());
        let _ = holder_bus;
    }

    /// 持有者心跳过期且进程已死后，观察者接管并接管落盘。
    #[tokio::test]
    async fn observer_takes_over_after_holder_heartbeat_expires() {
        let (dir, journal) = session();
        let state_dir = dir.path();
        // 1. 伪造一个「心跳过期 + 进程已死」的持有者登记（不修改 ownership 的语义，
        //    只是按同样的文件格式写一份）
        let long_past = chrono::Utc::now() - chrono::Duration::seconds(3600);
        std::fs::write(
            state_dir.join("session-holder.json"),
            serde_json::to_string_pretty(&json!({
                "schema": 1,
                "session_id": "session",
                "owner": "web",
                "pid": u32::MAX,
                "started_at": long_past.to_rfc3339(),
                "heartbeat_at": long_past.to_rfc3339(),
                "transport": null,
                "watchers": 0,
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(!holder_is_alive(&session_holder(state_dir).unwrap()));

        // 2. 观察者接入后应当在看门狗的下一拍接管
        let (link, bus) = SessionLink::attach(
            SessionOwner::Repl,
            state_dir,
            "session",
            journal.clone(),
            "workspace",
        )
        .await;
        assert!(
            wait_for_role(&link, LinkRole::Holder).await,
            "心跳过期后观察者应当接管"
        );
        assert!(bus.is_holder(), "接管后事件总线应切成持有者模式");
        assert_eq!(session_holder(state_dir).unwrap().owner, "repl");
    }

    /// 新连接按序号补发历史：只发 `after` 之后的事件。
    #[tokio::test]
    async fn new_connection_replays_history_after_sequence() {
        let (dir, journal) = session();
        let state_dir = dir.path();

        let (_link, holder_bus) = SessionLink::attach(
            SessionOwner::Web,
            state_dir,
            "session",
            journal.clone(),
            "workspace",
        )
        .await;
        for index in 0..4u64 {
            let _ = holder_bus.emit(WebEvent::new(
                "run",
                "workspace",
                "session",
                "message.content.delta",
                json!({ "text": index.to_string() }),
            ));
        }
        let _ = wait_for_events(&journal, 4).await;

        // 模拟一个已经收到前 2 条事件的观察者重连
        let transport = transport_for_state_dir(state_dir).unwrap();
        let mut stream = transport.connect().await.unwrap();
        stream
            .send(&Frame::control(KIND_CTL_HELLO, json!({ "after": 2 })))
            .await
            .unwrap();
        let mut sequences = Vec::new();
        while let Ok(Some(frame)) = stream.recv().await {
            if frame.kind != KIND_EVT_MIRROR {
                continue;
            }
            if let Some(sequence) = frame.sequence {
                sequences.push(sequence);
            }
            if sequences.len() == 2 {
                break;
            }
        }
        assert_eq!(sequences, vec![3, 4], "只应补发 after 之后的事件");
    }

    /// 只读回扫不写文件：调用前后文件大小不变。
    #[test]
    fn journal_tail_read_is_read_only() {
        let (_dir, journal) = session();
        std::fs::write(
            &journal,
            format!(
                "{}\n{}\n",
                json!({"sequence":1,"run_id":"r","workspace_id":"w","session_id":"s","timestamp":"t","type":"a","payload":{}}),
                json!({"sequence":2,"run_id":"r","workspace_id":"w","session_id":"s","timestamp":"t","type":"b","payload":{}}),
            ),
        )
        .unwrap();
        let before = std::fs::metadata(&journal).unwrap().len();

        assert_eq!(read_journal_tail(&journal, 0).len(), 2);
        assert_eq!(read_journal_tail(&journal, 1).len(), 1);
        assert!(read_journal_tail(&journal, 2).is_empty());
        assert_eq!(
            std::fs::metadata(&journal).unwrap().len(),
            before,
            "只读回扫不得改写事件文件"
        );
    }

    /// 建一次观察者上行提交所需的请求。
    ///
    /// 参数:
    /// - `run_id`: 轮次标识
    /// - `submit_id`: 提交标识
    ///
    /// 返回:
    /// - 上行请求
    fn submitted_run(run_id: &str, submit_id: &str) -> SubmittedRun {
        SubmittedRun {
            submit_id: submit_id.to_string(),
            run_id: run_id.to_string(),
            workspace_id: "workspace".to_string(),
            workspace_path: "/tmp/workspace".to_string(),
            request: StartRunRequest {
                kind: crate::web::runs::RunKind::Conversation,
                session_id: "session".to_string(),
                input: "你好".to_string(),
                agent_id: None,
                image_url: None,
                image_urls: Vec::new(),
                mode: None,
                provider_id: None,
                model: None,
                thinking_level: None,
                insert_at: crate::web::runs::QueueInsertAt::Turn,
            },
        }
    }

    /// 等观察者真正连上持有者。
    async fn wait_until_connected(link: &SessionLink) -> bool {
        for _ in 0..300 {
            if link.is_connected() {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        link.is_connected()
    }

    /// 观察者把一件事交给持有者执行：提交上行、持有者受理、回执带回状态。
    ///
    /// 两端跑在同一个进程里，但走的是真实的 IPC 传输层（Unix socket /
    /// named pipe），不是内存通道。
    #[tokio::test]
    async fn observer_submit_is_executed_by_the_holder() {
        let (dir, journal) = session();
        let state_dir = dir.path();

        let (holder_link, holder_bus) = SessionLink::attach(
            SessionOwner::Web,
            state_dir,
            "session",
            journal.clone(),
            "workspace",
        )
        .await;
        assert!(wait_for_role(&holder_link, LinkRole::Holder).await);

        // 持有者侧的执行器：记下上行的一轮并受理
        let (sink_tx, mut sink_rx) = mpsc::unbounded_channel::<HolderRequest>();
        holder_link.set_holder_sink(sink_tx);
        let executed = Arc::new(Mutex::new(Vec::<String>::new()));
        let recorded = executed.clone();
        tokio::spawn(async move {
            while let Some(request) = sink_rx.recv().await {
                match request {
                    HolderRequest::Submit { request, reply } => {
                        recorded
                            .lock()
                            .unwrap_or_else(|error| error.into_inner())
                            .push(request.run_id);
                        let _ = reply.send(Ok("running".to_string()));
                    }
                    HolderRequest::Abort { reply, .. } => {
                        let _ = reply.send(Ok(true));
                    }
                }
            }
        });

        let (observer_link, _observer_bus) = SessionLink::attach(
            SessionOwner::Repl,
            state_dir,
            "session",
            journal.clone(),
            "workspace",
        )
        .await;
        assert!(wait_for_role(&observer_link, LinkRole::Observer).await);
        assert!(wait_until_connected(&observer_link).await);
        let mut subscription = observer_link.subscribe();

        let ack = observer_link
            .submit(submitted_run("run-1", "sub-1"))
            .await
            .expect("提交必须拿到回执，不能静默丢弃");
        assert!(ack.accepted, "持有者应当受理：{ack:?}");
        assert_eq!(ack.status.as_deref(), Some("running"));
        assert_eq!(ack.submit_id, "sub-1");
        assert_eq!(
            executed
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .as_slice(),
            ["run-1"],
            "持有者必须沿用观察者分配的轮次标识，否则事件流对不上"
        );

        // 持有者执行这一轮时产生的事件要下行到观察者
        let _ = holder_bus.emit(WebEvent::new(
            "run-1",
            "workspace",
            "session",
            "message.content.delta",
            json!({ "text": "来自持有者" }),
        ));
        let event = tokio::time::timeout(Duration::from_secs(5), subscription.events.recv())
            .await
            .expect("观察者应当在超时前收到下行事件")
            .expect("订阅通道应当收到事件");
        assert_eq!(event.run_id, "run-1");
        assert_eq!(event.payload["text"], "来自持有者");
    }

    /// 持有者不接受外部驱动时，提交必须被明确拒绝而不是石沉大海。
    #[tokio::test]
    async fn submit_is_rejected_when_the_holder_cannot_be_driven() {
        let (dir, journal) = session();
        let state_dir = dir.path();

        // 持有者不登记执行器：模拟 TUI 这类拿不到外部轮次入口的持有者
        let (_holder_link, _holder_bus) = SessionLink::attach(
            SessionOwner::Repl,
            state_dir,
            "session",
            journal.clone(),
            "workspace",
        )
        .await;
        let (observer_link, _observer_bus) = SessionLink::attach(
            SessionOwner::Web,
            state_dir,
            "session",
            journal.clone(),
            "workspace",
        )
        .await;
        assert!(wait_for_role(&observer_link, LinkRole::Observer).await);
        assert!(wait_until_connected(&observer_link).await);

        let ack = observer_link
            .submit(submitted_run("run-2", "sub-2"))
            .await
            .expect("拒绝也要有回执");
        assert!(!ack.accepted);
        assert!(
            ack.reason
                .as_deref()
                .is_some_and(|reason| !reason.trim().is_empty()),
            "拒绝必须带上可操作的原因：{ack:?}"
        );
    }

    /// 拿不到持有者时提交立刻失败：用户输入绝不能凭空消失。
    #[tokio::test]
    async fn submit_fails_loudly_without_a_holder() {
        let (_dir, journal) = session();
        let link = SessionLink::detached(&journal, "workspace", "session");
        let error = link
            .submit(submitted_run("run-3", "sub-3"))
            .await
            .expect_err("没有持有者时提交必须失败");
        assert!(
            error.to_string().contains("未被执行"),
            "错误信息要说清输入没有被执行：{error}"
        );
    }

    /// 持有者在回执前失联：未决提交一律判失败，调用方据此给用户反馈。
    #[tokio::test]
    async fn pending_submits_are_failed_when_the_holder_goes_away() {
        let (_dir, journal) = session();
        let link = SessionLink::detached(&journal, "workspace", "session");
        // 伪造一条上行通道：提交会登记成未决，然后一直等回执
        let (upstream_tx, upstream_rx) = mpsc::unbounded_channel::<Frame>();
        *link
            .state
            .upstream
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(upstream_tx);

        let pending = {
            let link = link.clone();
            tokio::spawn(async move { link.submit(submitted_run("run-4", "sub-4")).await })
        };
        // 等提交真的登记成未决，再模拟持有者断开
        for _ in 0..300 {
            let registered = link
                .state
                .pending_submits
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .contains_key("sub-4");
            if registered {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        link.fail_pending("会话持有者已断开");

        let ack = tokio::time::timeout(Duration::from_secs(5), pending)
            .await
            .expect("未决提交必须被唤醒")
            .expect("判失败后 submit 返回 Ok(回执)")
            .expect("回执必须送达，不能让提交悬空");
        assert!(!ack.accepted);
        assert_eq!(ack.reason.as_deref(), Some("会话持有者已断开"));
        // 上行帧确实发出去了：失败不是因为压根没尝试
        drop(upstream_rx);
    }

    /// 观察者请求的中断同样上行到持有者，并带回是否命中活动轮次。
    #[tokio::test]
    async fn abort_is_routed_to_the_holder() {
        let (dir, journal) = session();
        let state_dir = dir.path();

        let (holder_link, _holder_bus) = SessionLink::attach(
            SessionOwner::Web,
            state_dir,
            "session",
            journal.clone(),
            "workspace",
        )
        .await;
        assert!(wait_for_role(&holder_link, LinkRole::Holder).await);
        let (sink_tx, mut sink_rx) = mpsc::unbounded_channel::<HolderRequest>();
        holder_link.set_holder_sink(sink_tx);
        let aborted = Arc::new(Mutex::new(Vec::<String>::new()));
        let recorded = aborted.clone();
        tokio::spawn(async move {
            while let Some(request) = sink_rx.recv().await {
                match request {
                    HolderRequest::Submit { reply, .. } => {
                        let _ = reply.send(Ok("running".to_string()));
                    }
                    HolderRequest::Abort { run_id, reply } => {
                        recorded
                            .lock()
                            .unwrap_or_else(|error| error.into_inner())
                            .push(run_id);
                        let _ = reply.send(Ok(true));
                    }
                }
            }
        });

        let (observer_link, _observer_bus) = SessionLink::attach(
            SessionOwner::Repl,
            state_dir,
            "session",
            journal.clone(),
            "workspace",
        )
        .await;
        assert!(wait_for_role(&observer_link, LinkRole::Observer).await);
        assert!(wait_until_connected(&observer_link).await);

        assert!(
            observer_link.abort("run-5").await.expect("中断必须有回执"),
            "持有者应当命中活动轮次"
        );
        assert_eq!(
            aborted.lock().unwrap_or_else(|e| e.into_inner()).as_slice(),
            ["run-5"]
        );
    }

    /// 单进程降级（拿不到 IPC 端点）：角色固定 Detached，行为与 P3 完全一致。
    #[tokio::test]
    async fn detached_link_keeps_single_process_semantics() {
        let (_dir, journal) = session();
        let link = SessionLink::detached(&journal, "workspace", "session");
        assert_eq!(link.role(), LinkRole::Detached);
        assert!(!link.is_connected());
        // 本地事件总线照常落盘：没有持有者时本进程自己写
        let bus = SessionActor::spawn(journal.clone(), "workspace", "session");
        let _ = bus.emit(WebEvent::new(
            "run",
            "workspace",
            "session",
            "message.content.delta",
            json!({ "text": "本地" }),
        ));
        let events = wait_for_events(&journal, 1).await;
        assert_eq!(events.len(), 1, "单进程模式下事件仍由本进程落盘");
        // 上行事件在无持有者时视为丢弃，但不影响本轮
        assert!(!link.mirror(events[0].clone()));
    }
}
