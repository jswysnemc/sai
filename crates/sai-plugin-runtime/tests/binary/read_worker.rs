use anyhow::{bail, Result};
use async_trait::async_trait;
use sai_plugin_runtime::{host::*, Capabilities};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Condvar, Mutex,
};
use std::time::Duration;

#[derive(Default)]
pub struct Worker {
    open: Mutex<bool>,
    ready: Condvar,
    pub entered: tokio::sync::Notify,
    pub released: tokio::sync::Notify,
}

impl Worker {
    /// 【二进制读取测试】【线程阻塞】用条件变量模拟已经进入内核、不能立即取消的读取
    /// @returns 测试主动释放时成功，兜底超时防止断言失败后遗留线程
    fn wait(&self) -> Result<()> {
        self.entered.notify_one();
        let (_guard, timeout) = self
            .ready
            .wait_timeout_while(self.open.lock().unwrap(), Duration::from_secs(5), |open| {
                !*open
            })
            .unwrap();
        if timeout.timed_out() {
            bail!("fixture worker was not released");
        }
        Ok(())
    }

    /// 【二进制读取测试】【线程放行】允许真实阻塞线程退出并释放其预算
    /// @returns 无；后续等待不会再次阻塞
    pub fn release(&self) {
        *self.open.lock().unwrap() = true;
        self.ready.notify_all();
    }
}

pub struct BlockingHost {
    pub worker: Arc<Worker>,
    pub blocking: AtomicBool,
    pub calls: AtomicUsize,
}

impl BlockingHost {
    /// 【二进制读取测试】【阻塞宿主】创建可控制读取完成时机的宿主
    /// @returns 初始读取会停在真实阻塞线程中的实例
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            worker: Arc::new(Worker::default()),
            blocking: AtomicBool::new(true),
            calls: AtomicUsize::new(0),
        })
    }
}

#[async_trait]
impl PluginHost for BlockingHost {
    /// 【二进制读取测试】【网络隔离】线程生命周期测试不访问网络
    /// @param request 请求；capabilities 为授权；allow_writes 为权限
    /// @returns 固定错误
    async fn http(&self, _: HttpRequest, _: Capabilities, _: bool) -> Result<HttpResponse> {
        bail!("unexpected HTTP")
    }

    /// 【二进制读取测试】【预算转移】把预留缓冲移入线程，即使 Future 释放也继续占用额度
    /// @param path 路径；buffer 为预算；context 为可信目录；capabilities 为授权
    /// @returns 正常模式下返回字节，阻塞模式在线程退出后报告模拟读取失败
    async fn read_binary(
        &self,
        path: String,
        mut buffer: BinaryReadBuffer,
        _: SystemContext,
        capabilities: Capabilities,
    ) -> Result<BinaryData> {
        capabilities.system.check_read_request(&path)?;
        self.calls.fetch_add(1, Ordering::SeqCst);
        if !self.blocking.load(Ordering::SeqCst) {
            buffer.extend_from_slice(&[42; 16])?;
            return Ok(buffer.finish());
        }
        let worker = self.worker.clone();
        tokio::task::spawn_blocking(move || {
            let result = worker.wait();
            drop(buffer);
            worker.released.notify_one();
            result?;
            bail!("fixture blocked read ended")
        })
        .await?
    }
}
