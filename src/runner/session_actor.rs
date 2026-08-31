use super::RunnerEvent;
use crate::ipc::frame::{Frame, KIND_EVT_MIRROR};
use crate::ipc::link::SessionLink;
use crate::web::runs::{EventAssembler, EventJournal, WebEvent};
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::sync::mpsc;

/// 观察者的事件缓冲容量。
///
/// 本地与远程观察者共用：缓冲满了就摘除消费者并留 lagged 标记，
/// 让客户端按最后收到的序号从落盘日志补发，而不是把事件静默丢掉。
const WATCHER_CAPACITY: usize = 1024;

/// 会话事件总线的命令。
pub(crate) enum ActorCmd {
    /// 开启新一轮：重置组装器的轮次边界状态并记录该轮的用户输入。
    BeginRun {
        run_id: String,
        input: String,
        image_urls: Vec<String>,
    },
    /// 把 runner 事件组装成 Web 事件后发布。
    Publish(RunnerEvent),
    /// 发布一条已经组装好的控制类 Web 事件。
    Emit(WebEvent),
    /// 发布一条其它进程经 IPC 上行、已经组装好的事件。
    ///
    /// 与 [`ActorCmd::Publish`] 的区别：它不再经过本进程的组装器——
    /// 对端已经组装完毕，这里只负责分配序号、落盘并扇出。
    Mirror(WebEvent),
    /// 附加一个观察者。
    Attach(Watcher),
}

/// 会话事件观察者。
pub(crate) enum Watcher {
    /// 本进程内的订阅者（SSE 流等）。
    Local {
        tx: mpsc::Sender<WebEvent>,
        /// 因通道已满而丢弃的事件数量；订阅端持有同一计数以便提示前端。
        dropped: Arc<AtomicUsize>,
    },
    /// 经 IPC 连接的其它进程。
    ///
    /// 投递只把帧放进有界 `mpsc`，真正的 socket 写入由连接任务在另一个 task 完成。
    /// 这是必须的：[`Watcher::deliver`] 是同步的（扇出发生在事件总线的串行循环里），
    /// 而 socket 写入是异步的——在 `deliver` 里 await 会让一个慢网络观察者
    /// 卡住整个会话的事件总线。
    Remote {
        tx: mpsc::Sender<Frame>,
        dropped: Arc<AtomicUsize>,
    },
}

/// 会话事件订阅。
pub(crate) struct SessionSubscription {
    /// 实时事件接收端；观察者被摘除后返回空，客户端应带序号重连。
    pub(crate) events: mpsc::Receiver<WebEvent>,
    /// 因消费者跟不上而丢弃的事件数量。
    pub(crate) dropped: Arc<AtomicUsize>,
}

impl SessionSubscription {
    /// 返回自订阅以来因消费者跟不上而丢弃的事件数量。
    ///
    /// 返回:
    /// - 丢弃数量；非零表示存在事件空洞，需要按最后收到的序号补发
    pub(crate) fn dropped_events(&self) -> usize {
        self.dropped.load(Ordering::Relaxed)
    }
}

impl Watcher {
    /// 创建本地观察者，并与订阅端共享丢弃计数。
    ///
    /// 参数:
    /// - `capacity`: 事件缓冲容量
    ///
    /// 返回:
    /// - 观察者与订阅端
    pub(crate) fn local(capacity: usize) -> (Self, SessionSubscription) {
        let (tx, events) = mpsc::channel(capacity);
        let dropped = Arc::new(AtomicUsize::new(0));
        (
            Self::Local {
                tx,
                dropped: dropped.clone(),
            },
            SessionSubscription { events, dropped },
        )
    }

    /// 创建远程观察者。
    ///
    /// 参数:
    /// - `capacity`: 帧缓冲容量
    ///
    /// 返回:
    /// - 观察者、帧接收端（交给连接任务写 socket）与丢弃计数
    pub(crate) fn remote(capacity: usize) -> (Self, mpsc::Receiver<Frame>, Arc<AtomicUsize>) {
        let (tx, frames) = mpsc::channel(capacity);
        let dropped = Arc::new(AtomicUsize::new(0));
        (
            Self::Remote {
                tx,
                dropped: dropped.clone(),
            },
            frames,
            dropped,
        )
    }

    /// 投递事件。
    ///
    /// 通道满时不再等待：慢消费者会被摘除并留下 lagged 标记，前端重连时
    /// 按最后收到的序号从落盘日志补发，避免像 broadcast 那样静默丢事件。
    ///
    /// 参数:
    /// - `event`: 已落盘并分配序号的事件
    ///
    /// 返回:
    /// - 观察者是否仍然有效
    pub(crate) fn deliver(&mut self, event: &WebEvent) -> bool {
        match self {
            Self::Local { tx, dropped } => match tx.try_send(event.clone()) {
                Ok(()) => true,
                Err(mpsc::error::TrySendError::Full(_)) => {
                    dropped.fetch_add(1, Ordering::Relaxed);
                    false
                }
                Err(mpsc::error::TrySendError::Closed(_)) => false,
            },
            Self::Remote { tx, dropped } => match tx.try_send(remote_frame(event)) {
                Ok(()) => true,
                Err(mpsc::error::TrySendError::Full(_)) => {
                    dropped.fetch_add(1, Ordering::Relaxed);
                    false
                }
                Err(mpsc::error::TrySendError::Closed(_)) => false,
            },
        }
    }
}

/// 把已落盘的事件编码成下行帧。
///
/// 参数:
/// - `event`: 已分配序号的事件
///
/// 返回:
/// - 待写入 IPC 的帧
fn remote_frame(event: &WebEvent) -> Frame {
    Frame {
        kind: KIND_EVT_MIRROR.to_string(),
        sequence: Some(event.sequence),
        payload: serde_json::to_value(event).unwrap_or(Value::Null),
    }
}

/// 会话事件执行者。
///
/// 单任务串行处理命令，保证同一会话的事件序号单调递增，并向所有观察者
/// 扇出同一份事件。本阶段 Agent 仍按轮次创建，执行者只负责事件总线。
pub(crate) struct SessionActor {
    journal: EventJournal,
    assembler: EventAssembler,
    watchers: Vec<Watcher>,
    cmds: mpsc::UnboundedReceiver<ActorCmd>,
}

/// 事件总线的角色实现。
///
/// 同一个句柄可以在运行期从观察者升级为持有者（见 [`ActorHandle::promote_to_holder`]），
/// 所以这里放在锁后面而不是让句柄直接持有某一个实现。
enum ActorInner {
    /// 本进程是会话持有者：事件落盘后扇出给本地与远程观察者。
    Local {
        tx: mpsc::UnboundedSender<ActorCmd>,
        journal: EventJournal,
    },
    /// 本进程是观察者：事件经 IPC 上行给持有者，本进程不写事件文件。
    Remote(RemoteBus),
}

/// 观察者侧的事件总线：本地组装后上行，落盘与序号分配都交给持有者。
///
/// 这样整个会话的事件文件始终只有一个写者，两个进程各自累加的序号不会撞车。
struct RemoteBus {
    assembler: EventAssembler,
    link: SessionLink,
}

/// 会话事件总线句柄。
///
/// 可克隆。`publish` / `emit` 只入队不 await，因此可以直接用作同步 sink。
#[derive(Clone)]
pub(crate) struct ActorHandle {
    inner: Arc<Mutex<ActorInner>>,
}

impl SessionActor {
    /// 启动会话事件总线（持有者模式）。
    ///
    /// 参数:
    /// - `path`: 会话事件日志文件
    /// - `workspace_id`: 工作区标识
    /// - `session_id`: 会话标识
    ///
    /// 返回:
    /// - 事件总线句柄
    pub(crate) fn spawn(path: PathBuf, workspace_id: &str, session_id: &str) -> ActorHandle {
        let journal = EventJournal::persistent(path);
        Self::spawn_with_journal(journal, workspace_id, session_id)
    }

    /// 用指定事件日志启动会话事件总线。
    ///
    /// 参数:
    /// - `journal`: 事件日志
    /// - `workspace_id`: 工作区标识
    /// - `session_id`: 会话标识
    ///
    /// 返回:
    /// - 事件总线句柄
    fn spawn_with_journal(
        journal: EventJournal,
        workspace_id: &str,
        session_id: &str,
    ) -> ActorHandle {
        let (tx, cmds) = mpsc::unbounded_channel();
        let actor = Self {
            journal: journal.clone(),
            assembler: EventAssembler::new(workspace_id, session_id),
            watchers: Vec::new(),
            cmds,
        };
        tokio::spawn(actor.run());
        ActorHandle::holder(tx, journal)
    }

    /// 串行消费命令直到句柄全部释放。
    async fn run(mut self) {
        while let Some(command) = self.cmds.recv().await {
            match command {
                ActorCmd::BeginRun {
                    run_id,
                    input,
                    image_urls,
                } => self.assembler.begin_run(&run_id, &input, &image_urls),
                ActorCmd::Publish(event) => {
                    let events = self.assembler.map(event);
                    for event in events {
                        self.emit(event);
                    }
                }
                ActorCmd::Emit(event) => self.emit(event),
                ActorCmd::Mirror(event) => self.emit(event),
                ActorCmd::Attach(watcher) => self.watchers.push(watcher),
            }
        }
    }

    /// 落盘一次并向所有观察者扇出。
    fn emit(&mut self, event: WebEvent) {
        let event = self.journal.publish(event);
        self.watchers.retain_mut(|watcher| watcher.deliver(&event));
    }
}

impl ActorHandle {
    /// 构造持有者侧句柄。
    fn holder(tx: mpsc::UnboundedSender<ActorCmd>, journal: EventJournal) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ActorInner::Local { tx, journal })),
        }
    }

    /// 构造观察者侧句柄。
    ///
    /// 参数:
    /// - `workspace_id`: 工作区标识
    /// - `session_id`: 会话标识
    /// - `link`: 该会话的跨进程链接
    ///
    /// 返回:
    /// - 事件总线句柄
    pub(crate) fn observer(workspace_id: &str, session_id: &str, link: SessionLink) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ActorInner::Remote(RemoteBus {
                assembler: EventAssembler::new(workspace_id, session_id),
                link,
            }))),
        }
    }

    /// 返回当前角色实现。
    fn lock(&self) -> MutexGuard<'_, ActorInner> {
        self.inner.lock().unwrap_or_else(|error| error.into_inner())
    }

    /// 返回会话事件日志，供补发历史与回读使用。
    ///
    /// 仅持有者模式下有意义：观察者不持有事件文件的写视图，这里返回一个空日志。
    /// 需要读历史请用 [`Self::replay`]——它只读地扫描共享文件尾部，不会像
    /// `EventJournal::persistent` 那样在超限时重写文件、把持有者刚追加的事件丢掉。
    pub(crate) fn journal(&self) -> EventJournal {
        match &*self.lock() {
            ActorInner::Local { journal, .. } => journal.clone(),
            ActorInner::Remote(_) => EventJournal::new(),
        }
    }

    /// 返回指定序号之后的历史事件。
    ///
    /// 持有者从内存里的有界日志补发；观察者只读地扫描共享文件尾部，
    /// 因此持有者失联、本进程降级期间仍然能补齐空洞。
    ///
    /// 参数:
    /// - `after`: 已接收的最后事件序号
    ///
    /// 返回:
    /// - 需要补发的事件
    pub(crate) fn replay(&self, after: u64) -> Vec<WebEvent> {
        match &*self.lock() {
            ActorInner::Local { journal, .. } => journal.events_after(after),
            ActorInner::Remote(bus) => bus.link.replay(after),
        }
    }

    /// 本进程是否是该会话的持有者。
    pub(crate) fn is_holder(&self) -> bool {
        matches!(&*self.lock(), ActorInner::Local { .. })
    }

    /// 开启新一轮，重置轮次边界状态。
    ///
    /// 参数:
    /// - `run_id`: 运行标识
    /// - `input`: 本轮用户输入
    /// - `image_urls`: 本轮图片列表
    ///
    /// 返回:
    /// - 入队结果
    pub(crate) fn begin_run(&self, run_id: &str, input: &str, image_urls: &[String]) -> Result<()> {
        match &mut *self.lock() {
            ActorInner::Local { tx, .. } => tx
                .send(ActorCmd::BeginRun {
                    run_id: run_id.to_string(),
                    input: input.to_string(),
                    image_urls: image_urls.to_vec(),
                })
                .map_err(|_| anyhow!("session event bus is closed")),
            // 观察者自己组装，持有者不需要知道轮次边界
            ActorInner::Remote(bus) => {
                bus.assembler.begin_run(run_id, input, image_urls);
                Ok(())
            }
        }
    }

    /// 入队一条 runner 事件，由执行者组装后统一发布。
    ///
    /// 参数:
    /// - `event`: runner 事件
    ///
    /// 返回:
    /// - 入队结果
    pub(crate) fn publish(&self, event: RunnerEvent) -> Result<()> {
        match &mut *self.lock() {
            ActorInner::Local { tx, .. } => tx
                .send(ActorCmd::Publish(event))
                .map_err(|_| anyhow!("session event bus is closed")),
            ActorInner::Remote(bus) => {
                for event in bus.assembler.map(event) {
                    bus.link.mirror(event);
                }
                // 上行失败不判本轮失败：对端挂了只影响同步，不影响本轮对话
                Ok(())
            }
        }
    }

    /// 入队一条已组装的事件。
    ///
    /// 参数:
    /// - `event`: Web 事件
    ///
    /// 返回:
    /// - 入队结果
    pub(crate) fn emit(&self, event: WebEvent) -> Result<()> {
        match &*self.lock() {
            ActorInner::Local { tx, .. } => tx
                .send(ActorCmd::Emit(event))
                .map_err(|_| anyhow!("session event bus is closed")),
            ActorInner::Remote(bus) => {
                bus.link.mirror(event);
                Ok(())
            }
        }
    }

    /// 发布一条来自其它进程的已组装事件。
    ///
    /// 持有者侧由 IPC 连接任务调用；观察者侧退化成上行（不应发生，保留兜底）。
    ///
    /// 参数:
    /// - `event`: 已组装的事件
    ///
    /// 返回:
    /// - 入队结果
    pub(crate) fn mirror(&self, event: WebEvent) -> Result<()> {
        match &*self.lock() {
            ActorInner::Local { tx, .. } => tx
                .send(ActorCmd::Mirror(event))
                .map_err(|_| anyhow!("session event bus is closed")),
            ActorInner::Remote(bus) => {
                bus.link.mirror(event);
                Ok(())
            }
        }
    }

    /// 附加一个本地观察者，或订阅持有者的事件流。
    ///
    /// 返回:
    /// - 会话事件订阅；执行者已停止时返回空
    pub(crate) fn attach(&self) -> Option<SessionSubscription> {
        match &mut *self.lock() {
            ActorInner::Local { tx, .. } => {
                let (watcher, subscription) = Watcher::local(WATCHER_CAPACITY);
                tx.send(ActorCmd::Attach(watcher)).ok()?;
                Some(subscription)
            }
            ActorInner::Remote(bus) => Some(bus.link.subscribe()),
        }
    }

    /// 附加一个远程观察者（持有者侧由 IPC 连接任务调用）。
    ///
    /// 返回的接收端交给连接任务写 socket；观察者被摘除后它返回 `None`，
    /// 连接任务据此结束这条连接。
    ///
    /// 参数:
    /// - `capacity`: 帧缓冲容量
    ///
    /// 返回:
    /// - 帧接收端与丢弃计数；本进程不是持有者时返回空
    pub(crate) fn attach_remote(
        &self,
        capacity: usize,
    ) -> Option<(mpsc::Receiver<Frame>, Arc<AtomicUsize>)> {
        match &*self.lock() {
            ActorInner::Local { tx, .. } => {
                let (watcher, frames, dropped) = Watcher::remote(capacity);
                tx.send(ActorCmd::Attach(watcher)).ok()?;
                Some((frames, dropped))
            }
            ActorInner::Remote(_) => None,
        }
    }

    /// 把观察者句柄就地升级为持有者（failover 接管）。
    ///
    /// 事件日志从文件尾部接续：接管前由旧持有者落盘的事件不会与新一轮重号。
    /// 升级后原来经 IPC 订阅的观察者仍然收到事件——本地总线会桥接回链接的扇出中心，
    /// 否则已经在看这个会话的 SSE 流会在接管瞬间变哑。
    ///
    /// 参数:
    /// - `journal_path`: 会话事件日志文件
    ///
    /// 返回:
    /// - 是否完成了升级；本进程已经是持有者时返回 false
    pub(crate) fn promote_to_holder(&self, journal_path: &Path) -> bool {
        let (workspace_id, session_id, link) = {
            let inner = self.lock();
            match &*inner {
                ActorInner::Local { .. } => return false,
                ActorInner::Remote(bus) => (
                    bus.link.workspace_id(),
                    bus.link.session_id(),
                    bus.link.clone(),
                ),
            }
        };
        let journal = EventJournal::persistent(journal_path.to_path_buf());
        let (tx, cmds) = mpsc::unbounded_channel();
        let (bridge, bridge_subscription) = Watcher::local(WATCHER_CAPACITY);
        let mut bridge_rx = bridge_subscription.events;
        let actor = SessionActor {
            journal: journal.clone(),
            assembler: EventAssembler::new(&workspace_id, &session_id),
            watchers: vec![bridge],
            cmds,
        };
        // 接管后旧订阅者继续从扇出中心收事件，避免已打开的 SSE 流在接管瞬间变哑
        let fanout = link.clone();
        tokio::spawn(async move {
            while let Some(event) = bridge_rx.recv().await {
                fanout.post(event);
            }
        });
        tokio::spawn(actor.run());
        *self.lock() = ActorInner::Local { tx, journal };
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::ChatStreamChunk;
    use crate::llm::ChatStreamKind;
    use crate::runner::AutomaticInputEvent;
    use crate::runner::AutomaticInputKind;
    use serde_json::json;

    /// 创建测试用会话事件总线。
    fn bus() -> (ActorHandle, tempfile::TempDir) {
        let temp = tempfile::tempdir().unwrap();
        let handle = SessionActor::spawn(temp.path().join("session.jsonl"), "workspace", "session");
        (handle, temp)
    }

    /// 读取观察者通道中当前可用的全部事件。
    fn drain(subscription: &mut SessionSubscription) -> Vec<WebEvent> {
        let mut events = Vec::new();
        while let Ok(event) = subscription.events.try_recv() {
            events.push(event);
        }
        events
    }

    /// 等待执行者把已入队的命令处理完。
    async fn settle(handle: &ActorHandle, expected: usize) {
        let journal = handle.journal();
        for _ in 0..200 {
            if journal.events_after(0).len() >= expected {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    }

    /// 验证事件会落盘一次并扇出到全部观察者。
    #[tokio::test]
    async fn fans_out_published_events_to_every_watcher() {
        let (handle, _temp) = bus();
        let mut first = handle.attach().unwrap();
        let mut second = handle.attach().unwrap();

        handle
            .publish(RunnerEvent::AutomaticInput(AutomaticInputEvent::new(
                AutomaticInputKind::ExternalCompletion,
                "后台任务已完成".to_string(),
            )))
            .unwrap();
        settle(&handle, 2).await;

        let first_events = drain(&mut first);
        let second_events = drain(&mut second);
        assert_eq!(first_events.len(), second_events.len());
        assert!(first_events
            .iter()
            .any(|event| event.kind == "message.automatic.input"));
        assert_eq!(handle.journal().events_after(0).len(), first_events.len());
        assert_eq!(first.dropped_events(), 0);
    }

    /// 验证新一轮会重置组装器状态，避免上一轮状态渗入下一轮。
    #[tokio::test]
    async fn begins_a_new_run_with_reset_boundary_state() {
        let (handle, _temp) = bus();
        handle
            .publish(RunnerEvent::Agent(crate::agent::AgentEvent::Chunk(
                ChatStreamChunk {
                    kind: ChatStreamKind::Content,
                    text: "第一轮".to_string(),
                },
            )))
            .unwrap();
        handle.begin_run("run-2", "第二轮输入", &[]).unwrap();
        handle.publish(RunnerEvent::Started).unwrap();
        handle
            .publish(RunnerEvent::Agent(crate::agent::AgentEvent::Chunk(
                ChatStreamChunk {
                    kind: ChatStreamKind::Content,
                    text: "第二轮".to_string(),
                },
            )))
            .unwrap();
        settle(&handle, 5).await;

        let events = handle.journal().events_after(0);
        let statuses = events
            .iter()
            .filter(|event| event.kind == "status.changed")
            .map(|event| event.payload["status"].as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        // 轮次边界重置后，第二轮会重新发出 waiting_response / working
        assert_eq!(statuses, ["working", "waiting_response", "working"]);
        let started = events
            .iter()
            .find(|event| event.kind == "run.started")
            .unwrap();
        assert_eq!(started.run_id, "run-2");
        assert_eq!(started.payload["input"], "第二轮输入");
    }

    /// 验证观察者跟不上时发布不会被阻塞，而是留下 lagged 标记供前端按序号补发。
    #[tokio::test]
    async fn marks_watcher_lagged_instead_of_blocking() {
        let (handle, _temp) = bus();
        let mut subscription = handle.attach().unwrap();
        let total = WATCHER_CAPACITY * 2;
        for sequence in 0..total {
            handle
                .emit(WebEvent::new(
                    "run",
                    "workspace",
                    "session",
                    "message.content.delta",
                    json!({ "text": sequence.to_string() }),
                ))
                .unwrap();
        }
        settle(&handle, total).await;

        // 1. 慢消费者被摘除前仍收到缓冲区内的事件
        let delivered = drain(&mut subscription);
        assert!(delivered.len() <= WATCHER_CAPACITY);
        // 2. 摘除后留下丢弃计数，前端可据此按最后收到的序号重连补发
        assert!(subscription.dropped_events() > 0);
        // 3. 落盘不受慢消费者影响，重连后能补齐空洞
        assert!(handle.journal().events_after(0).len() >= WATCHER_CAPACITY);
        // 4. 摘除后接收端关闭，SSE 流结束并触发客户端重连
        handle
            .emit(WebEvent::new(
                "run",
                "workspace",
                "session",
                "run.completed",
                json!({}),
            ))
            .unwrap();
        settle(&handle, total + 1).await;
        assert!(drain(&mut subscription).is_empty());
        assert!(subscription.events.recv().await.is_none());
    }

    /// 远程观察者：事件被编码成带序号的帧，且不阻塞扇出。
    #[tokio::test]
    async fn fans_out_to_remote_watchers_as_numbered_frames() {
        let (handle, _temp) = bus();
        let (mut frames, dropped) = handle.attach_remote(16).unwrap();

        handle.begin_run("run-1", "你好", &[]).unwrap();
        handle
            .emit(WebEvent::new(
                "run-1",
                "workspace",
                "session",
                "message.content.delta",
                json!({ "text": "hi" }),
            ))
            .unwrap();
        settle(&handle, 1).await;

        let frame = frames.recv().await.expect("远程观察者应当收到帧");
        assert_eq!(frame.kind, KIND_EVT_MIRROR);
        assert_eq!(frame.sequence, Some(1));
        assert_eq!(frame.payload["type"], "message.content.delta");
        assert_eq!(frame.payload["payload"]["text"], "hi");
        assert_eq!(dropped.load(Ordering::Relaxed), 0);
    }

    /// 远程观察者跟不上时同样被摘除并留下 lagged 标记，扇出不被拖慢。
    #[tokio::test]
    async fn marks_slow_remote_watcher_lagged_instead_of_blocking() {
        let (handle, _temp) = bus();
        let capacity = 4;
        // 1. 全程不读接收端，模拟卡住的 socket
        let (frames, dropped) = handle.attach_remote(capacity).unwrap();
        let total = capacity * 4;
        for sequence in 0..total {
            handle
                .emit(WebEvent::new(
                    "run",
                    "workspace",
                    "session",
                    "message.content.delta",
                    json!({ "text": sequence.to_string() }),
                ))
                .unwrap();
        }
        settle(&handle, total).await;

        // 2. 摘除后留下丢弃计数，且接收端关闭让连接任务结束
        assert!(dropped.load(Ordering::Relaxed) > 0);
        drop(frames);
        // 3. 落盘不受影响：远程观察者重连后按序号补发即可补齐空洞
        assert_eq!(handle.journal().events_after(0).len(), total);
    }

    /// 新连接按序号补发历史：只发 `after` 之后的事件，序号严格递增。
    #[tokio::test]
    async fn replays_backlog_from_requested_sequence() {
        let (handle, _temp) = bus();
        for sequence in 0..6u64 {
            handle
                .emit(WebEvent::new(
                    "run",
                    "workspace",
                    "session",
                    "message.content.delta",
                    json!({ "text": sequence.to_string() }),
                ))
                .unwrap();
        }
        settle(&handle, 6).await;

        // 模拟一个已经收到前 4 条事件的观察者重连
        let backlog = handle.replay(4);
        let sequences = backlog
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>();
        assert_eq!(sequences, vec![5, 6]);
        assert!(handle.replay(6).is_empty());
    }

    /// 上行事件（观察者 → 持有者）不再经过组装器，直接落盘并扇出。
    #[tokio::test]
    async fn mirrors_remote_events_without_reassembling() {
        let (handle, _temp) = bus();
        let mut subscription = handle.attach().unwrap();

        handle
            .mirror(WebEvent::new(
                "run-remote",
                "workspace",
                "session",
                "message.content.delta",
                json!({ "text": "来自另一个前端" }),
            ))
            .unwrap();
        settle(&handle, 1).await;

        let events = drain(&mut subscription);
        assert_eq!(events.len(), 1, "上行事件不应被组装器拆成多条");
        assert_eq!(events[0].run_id, "run-remote");
        assert_eq!(events[0].payload["text"], "来自另一个前端");
    }
}
