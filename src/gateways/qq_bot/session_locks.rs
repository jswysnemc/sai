use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex, Weak};
use tokio::sync::{Mutex as AsyncMutex, RwLock};

/// 【QQ网关】【会话调度】同一目标串行，全局控制命令与消息处理互斥。
#[derive(Default)]
pub(super) struct SessionLocks {
    targets: Mutex<HashMap<String, Weak<AsyncMutex<()>>>>,
    control: RwLock<()>,
}

impl SessionLocks {
    /// 【QQ网关】【会话调度】在目标锁内执行任务；不同目标的普通消息可以并行。
    /// @param target 为目标类型与标识；control 表示会修改全局状态的控制命令；work 为任务
    /// @returns 任务返回值
    pub(super) async fn run<F: Future>(&self, target: String, control: bool, work: F) -> F::Output {
        let lock = {
            let mut targets = self
                .targets
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            // 1. 清理已完成目标，仅等待者和运行中的任务持有强引用
            targets.retain(|_, lock| lock.strong_count() > 0);
            match targets.get(&target).and_then(Weak::upgrade) {
                Some(lock) => lock,
                None => {
                    let lock = Arc::new(AsyncMutex::new(()));
                    targets.insert(target, Arc::downgrade(&lock));
                    lock
                }
            }
        };
        let _target = lock.lock().await;
        // 2. 控制命令沿用全局当前会话语义，执行期间不允许普通消息修改共享状态
        if control {
            let _control = self.control.write().await;
            work.await
        } else {
            let _control = self.control.read().await;
            work.await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::poll;
    use std::future::{pending, ready};
    use std::task::Poll;

    /// 【QQ网关】【调度回归】同目标排队、不同目标推进，释放后清理锁表；无参数或返回值。
    #[tokio::test]
    async fn unrelated_sessions_advance_while_same_session_waits() {
        let locks = SessionLocks::default();
        let mut first = Box::pin(locks.run("user:a".into(), false, pending::<()>()));
        assert!(poll!(&mut first).is_pending());
        let mut same = Box::pin(locks.run("user:a".into(), false, ready(1)));
        assert!(poll!(&mut same).is_pending());
        assert_eq!(locks.run("group:a".into(), false, ready(2)).await, 2);
        drop(first);
        assert_eq!(poll!(&mut same), Poll::Ready(1));
        drop(same);
        for index in 0..1000 {
            locks.run(index.to_string(), false, ready(())).await;
        }
        assert!(locks.targets.lock().unwrap().len() <= 1);
    }

    /// 【QQ网关】【控制互斥】全局命令等待活动消息，执行中阻止新消息；无参数或返回值。
    #[tokio::test]
    async fn control_commands_exclude_active_messages() {
        let locks = SessionLocks::default();
        let mut message = Box::pin(locks.run("user:a".into(), false, pending::<()>()));
        assert!(poll!(&mut message).is_pending());
        let mut command = Box::pin(locks.run("user:b".into(), true, pending::<()>()));
        assert!(poll!(&mut command).is_pending());
        drop(message);
        assert!(poll!(&mut command).is_pending());
        let mut next = Box::pin(locks.run("user:c".into(), false, ready(3)));
        assert!(poll!(&mut next).is_pending());
        drop(command);
        assert_eq!(poll!(&mut next), Poll::Ready(3));
    }
}
