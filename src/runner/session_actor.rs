use super::RunnerEvent;
use crate::web::runs::{EventAssembler, EventJournal, WebEvent};
use anyhow::{anyhow, Result};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

const WATCHER_CAPACITY: usize = 1024;

/// 【会话事件】【本地命令】只在当前进程内组装、保存和分发事件。
pub(crate) enum ActorCmd {
    BeginRun {
        run_id: String,
        input: String,
        image_urls: Vec<String>,
    },
    Publish(RunnerEvent),
    Emit(WebEvent),
    Attach(Watcher),
    AttachReady(Watcher, tokio::sync::oneshot::Sender<()>),
}

/// 【会话事件】【本地订阅】有界通道避免慢浏览器阻塞运行。
pub(crate) enum Watcher {
    Local {
        tx: mpsc::Sender<WebEvent>,
        dropped: Arc<AtomicUsize>,
    },
}

/// 【会话事件】【订阅句柄】接收同一 Web 服务中的会话事件。
pub(crate) struct SessionSubscription {
    pub(crate) events: mpsc::Receiver<WebEvent>,
    pub(crate) dropped: Arc<AtomicUsize>,
}

impl SessionSubscription {
    /// 【会话事件】【丢弃计数】无参数；返回需要从日志补发的事件数量。
    pub(crate) fn dropped_events(&self) -> usize {
        self.dropped.load(Ordering::Relaxed)
    }
}

impl Watcher {
    /// 【会话事件】【创建订阅】参数为通道容量；返回订阅者和接收句柄。
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

    /// 【会话事件】【非阻塞投递】参数为已落盘事件；返回订阅者是否仍可用。
    pub(crate) fn deliver(&mut self, event: &WebEvent) -> bool {
        let Self::Local { tx, dropped } = self;
        match tx.try_send(event.clone()) {
            Ok(()) => true,
            Err(mpsc::error::TrySendError::Full(_)) => {
                dropped.fetch_add(1, Ordering::Relaxed);
                false
            }
            Err(mpsc::error::TrySendError::Closed(_)) => false,
        }
    }
}

/// 【会话事件】【串行处理】保持同一服务中事件落盘与订阅顺序一致。
pub(crate) struct SessionActor {
    journal: EventJournal,
    assembler: EventAssembler,
    watchers: Vec<Watcher>,
    cmds: mpsc::UnboundedReceiver<ActorCmd>,
}

/// 【会话事件】【发送句柄】不含远程执行、持有者或角色切换逻辑。
#[derive(Clone)]
pub(crate) struct ActorHandle {
    tx: mpsc::UnboundedSender<ActorCmd>,
    journal: EventJournal,
}

impl SessionActor {
    /// 【会话事件】【启动】参数为日志路径、工作区与会话标识；返回本进程事件总线。
    pub(crate) fn spawn(path: PathBuf, workspace_id: &str, session_id: &str) -> ActorHandle {
        let journal = EventJournal::persistent(path);
        let (tx, cmds) = mpsc::unbounded_channel();
        let actor = Self {
            journal: journal.clone(),
            assembler: EventAssembler::new(workspace_id, session_id),
            watchers: Vec::new(),
            cmds,
        };
        tokio::spawn(actor.run());
        ActorHandle { tx, journal }
    }

    /// 【会话事件】【消费循环】无参数；全部发送句柄释放后结束。
    async fn run(mut self) {
        while let Some(command) = self.cmds.recv().await {
            match command {
                ActorCmd::BeginRun {
                    run_id,
                    input,
                    image_urls,
                } => self.assembler.begin_run(&run_id, &input, &image_urls),
                ActorCmd::Publish(event) => {
                    for event in self.assembler.map(event) {
                        self.emit(event);
                    }
                }
                ActorCmd::Emit(event) => self.emit(event),
                ActorCmd::Attach(watcher) => self.watchers.push(watcher),
                ActorCmd::AttachReady(watcher, ready) => {
                    self.watchers.push(watcher);
                    let _ = ready.send(());
                }
            }
        }
    }

    /// 【会话事件】【持久化】参数为事件；先落盘再分发，无返回值。
    fn emit(&mut self, event: WebEvent) {
        let event = self.journal.publish(event);
        self.watchers.retain_mut(|watcher| watcher.deliver(&event));
    }
}

impl ActorHandle {
    /// 【会话事件】【日志访问】无参数；返回本地日志句柄。
    pub(crate) fn journal(&self) -> EventJournal {
        self.journal.clone()
    }

    /// 【会话事件】【历史补发】参数为最后接收序号；返回该序号之后的事件。
    pub(crate) fn replay(&self, after: u64) -> Vec<WebEvent> {
        self.journal.events_after(after)
    }

    /// 【会话事件】【轮次边界】参数为运行标识、正文与图片；返回入队结果。
    pub(crate) fn begin_run(&self, run_id: &str, input: &str, image_urls: &[String]) -> Result<()> {
        self.send(ActorCmd::BeginRun {
            run_id: run_id.into(),
            input: input.into(),
            image_urls: image_urls.to_vec(),
        })
    }

    /// 【会话事件】【运行事件】参数为 Runner 事件；返回入队结果。
    pub(crate) fn publish(&self, event: RunnerEvent) -> Result<()> {
        self.send(ActorCmd::Publish(event))
    }

    /// 【会话事件】【控制事件】参数为已组装事件；返回入队结果。
    pub(crate) fn emit(&self, event: WebEvent) -> Result<()> {
        self.send(ActorCmd::Emit(event))
    }

    /// 【会话事件】【命令入队】参数为本地命令；返回通道发送结果。
    fn send(&self, command: ActorCmd) -> Result<()> {
        self.tx
            .send(command)
            .map_err(|_| anyhow!("session event bus is closed"))
    }

    /// 【会话事件】【订阅】无参数；返回本进程订阅，总线关闭时返回空。
    pub(crate) fn attach(&self) -> Option<SessionSubscription> {
        let (watcher, subscription) = Watcher::local(WATCHER_CAPACITY);
        self.send(ActorCmd::Attach(watcher)).ok()?;
        Some(subscription)
    }

    /// 【会话事件】【订阅确认】无参数；等待订阅安装后返回，保证读取历史与新事件间没有空洞。
    pub(crate) async fn attach_ready(&self) -> Option<SessionSubscription> {
        let (watcher, subscription) = Watcher::local(WATCHER_CAPACITY);
        let (send, receive) = tokio::sync::oneshot::channel();
        self.send(ActorCmd::AttachReady(watcher, send)).ok()?;
        receive.await.ok()?;
        Some(subscription)
    }
}

#[cfg(test)]
mod tests;
