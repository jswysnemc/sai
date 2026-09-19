use super::{completion::Snapshot, files, MentionSuggestion};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

/// 单次后台文件查询及用于验证结果的输入快照
pub(super) struct Request {
    pub(super) id: u64,
    pub(super) snapshot: Snapshot,
    pub(super) query: String,
}

/// 完成结果同时携带请求序号和输入快照
pub(super) struct Response {
    pub(super) id: u64,
    pub(super) snapshot: Snapshot,
    pub(super) items: Vec<MentionSuggestion>,
}

/// 最多保存一个等待请求和一个完成结果，防止快速输入堆积任务
#[derive(Default)]
struct Mailbox {
    request: Option<Request>,
    response: Option<Response>,
    closed: bool,
}

/// 【终端】【补全任务】单工作线程与容量固定的邮箱
pub(super) struct Worker {
    mailbox: Arc<(Mutex<Mailbox>, Condvar)>,
    generation: Arc<AtomicU64>,
}

impl Worker {
    /// 创建后台扫描线程，无参数；返回任务入口
    pub(super) fn new() -> Self {
        let mailbox = Arc::new((Mutex::new(Mailbox::default()), Condvar::new()));
        let generation = Arc::new(AtomicU64::new(0));
        let shared = mailbox.clone();
        let current = generation.clone();
        std::thread::spawn(move || {
            let mut cache = None;
            loop {
                // 1. 【终端】【补全任务】只读取最新请求，旧查询通过序号变化主动停止
                let request = {
                    let (lock, wake) = &*shared;
                    let mut mailbox = lock.lock().unwrap_or_else(|error| error.into_inner());
                    while mailbox.request.is_none() && !mailbox.closed {
                        mailbox = wake
                            .wait(mailbox)
                            .unwrap_or_else(|error| error.into_inner());
                    }
                    if mailbox.closed {
                        return;
                    }
                    mailbox.request.take().expect("pending request")
                };
                let cancelled = || current.load(Ordering::Acquire) != request.id;
                let Some(items) =
                    files::complete(&request.snapshot.cwd, &request.query, &mut cache, cancelled)
                else {
                    continue;
                };
                // 2. 【终端】【补全任务】取消后不发布；消费端仍会再次校验快照
                if !cancelled() {
                    shared
                        .0
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .response = Some(Response {
                        id: request.id,
                        snapshot: request.snapshot,
                        items,
                    });
                }
            }
        });
        Self {
            mailbox,
            generation,
        }
    }

    /// 取消旧任务并提交 request，返回无
    pub(super) fn submit(&self, request: Request) {
        self.cancel(request.id);
        self.mailbox
            .0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .request = Some(request);
        self.mailbox.1.notify_one();
    }

    /// 切换有效序号 id 并清理尚未执行的查询，返回无
    pub(super) fn cancel(&self, id: u64) {
        self.generation.store(id, Ordering::Release);
        let mut mailbox = self
            .mailbox
            .0
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        mailbox.request = None;
        mailbox.response = None;
    }

    /// 取出最近一次完成结果，无参数；尚未完成时返回空
    pub(super) fn take(&self) -> Option<Response> {
        self.mailbox
            .0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .response
            .take()
    }
}

impl Drop for Worker {
    /// 取消任务并唤醒线程退出，不等待可能阻塞的文件系统调用；无参数，无返回值
    fn drop(&mut self) {
        self.generation.fetch_add(1, Ordering::Release);
        self.mailbox
            .0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .closed = true;
        self.mailbox.1.notify_one();
    }
}
