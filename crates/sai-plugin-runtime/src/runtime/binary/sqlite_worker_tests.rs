use super::work;
use crate::{host::binary::BinaryBudget, ExecutionLimits};
use mlua::Lua;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

struct Exited(Arc<Notify>);

impl Drop for Exited {
    /// 【数据库取消测试】【线程完成】在预算守卫释放之后报告原生工作退出
    /// @returns 无；仅通知等待的测试调用
    fn drop(&mut self) {
        self.0.notify_one();
    }
}

#[derive(Clone, Copy)]
enum Stop {
    DropFuture,
    Timeout,
    CancelCallback,
}

/// 【数据库取消测试】【工作中断】在实际工作持有预留时中断等待或取消回调
/// @param stop 中断方式
/// @returns 无；退出前额度保持占用，退出后全部恢复且不交付迟到结果
async fn interrupted_worker(stop: Stop) {
    let lua = Lua::new();
    let cancel = CancellationToken::new();
    super::super::super::budget::install(&lua, &ExecutionLimits::default(), cancel.clone())
        .unwrap();
    let binary = BinaryBudget::new(4096);
    let held = binary.reserve(4096).unwrap();
    let entered = Arc::new(Notify::new());
    let exited = Arc::new(Notify::new());
    let completed = Arc::new(AtomicBool::new(false));
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let start = entered.clone();
    let finish = exited.clone();
    let published = completed.clone();
    let timeout = if matches!(stop, Stop::Timeout) {
        20
    } else {
        2000
    };
    let mut operation = Box::pin(work(&lua, timeout, move |budget| {
        let _exited = Exited(finish);
        let _held = held;
        start.notify_one();
        release_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        budget(0)?;
        published.store(true, Ordering::Release);
        Ok(())
    }));
    tokio::select! {
        _=entered.notified()=>{},
        result=&mut operation=>panic!("worker did not block: {result:?}"),
    }
    match stop {
        Stop::DropFuture => drop(operation),
        Stop::Timeout => {
            let error = operation.await.unwrap_err();
            assert!(error.to_string().contains("timed out"), "{error}");
        }
        Stop::CancelCallback => {
            cancel.cancel();
            assert_eq!(binary.available(), 0);
            release_tx.send(()).unwrap();
            let error = operation.await.unwrap_err();
            assert!(error.to_string().contains("cancelled"), "{error}");
            assert_eq!(binary.available(), 4096);
            assert!(!completed.load(Ordering::Acquire));
            return;
        }
    }
    assert_eq!(binary.available(), 0);
    release_tx.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(2), exited.notified())
        .await
        .unwrap();
    assert_eq!(binary.available(), 4096);
    assert!(!completed.load(Ordering::Acquire));
}

/// 【数据库取消测试】【等待释放】丢弃查询和修改共用的调度 Future，线程仍然持有预算
/// @returns 无；线程完成后才恢复全部可用字节
#[tokio::test]
async fn dropping_sqlite_work_waits_for_the_actual_worker_to_release_memory() {
    interrupted_worker(Stop::DropFuture).await;
}

/// 【数据库取消测试】【局部超时】单次操作超时取消实际工作但不提前回收额度
/// @returns 无；已经超时的工作不能完成结果交付
#[tokio::test]
async fn sqlite_work_timeout_cancels_computation_without_releasing_live_memory() {
    interrupted_worker(Stop::Timeout).await;
}

/// 【数据库取消测试】【回调取消】外围调用的取消信号由同一原生预算传播
/// @returns 无；工作感知回调已结束，不继续产生结果
#[tokio::test]
async fn callback_cancellation_reaches_running_sqlite_work() {
    interrupted_worker(Stop::CancelCallback).await;
}
