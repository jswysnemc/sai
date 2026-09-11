use anyhow::{bail, Result};
use async_trait::async_trait;
use sai_plugin_runtime::{host::*, Capabilities};
use std::sync::{
    atomic::{AtomicBool, Ordering},
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
    /// 【条件写入测试】【线程等待】模拟无法立即中断且继续使用缓冲的系统调用
    /// @returns 测试放行后成功，兜底时限避免失败后遗留线程
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

    /// 【条件写入测试】【线程放行】允许模拟系统调用完成并释放数据
    /// @returns 无；所有等待者可以继续执行
    pub fn release(&self) {
        *self.open.lock().unwrap() = true;
        self.ready.notify_all();
    }
}

pub struct BlockingHost {
    pub worker: Arc<Worker>,
    pub blocking: AtomicBool,
}

impl BlockingHost {
    /// 【条件写入测试】【阻塞宿主】创建初始阻塞的确定性宿主
    /// @returns 可共享实例及线程状态
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            worker: Arc::new(Worker::default()),
            blocking: AtomicBool::new(true),
        })
    }
}

#[async_trait]
impl PluginHost for BlockingHost {
    /// 【条件写入测试】【网络隔离】生命周期测试不得使用网络
    /// @param request 请求；capabilities 为授权；allow_writes 为权限
    /// @returns 固定错误
    async fn http(&self, _: HttpRequest, _: Capabilities, _: bool) -> Result<HttpResponse> {
        bail!("unexpected HTTP")
    }

    /// 【条件写入测试】【线程租约】实际工作线程持有缓冲，Future 取消不缩短数据生命周期
    /// @param request 条件；data 为预算租约；context 为上下文；capabilities 为授权
    /// @returns 模拟完成结果，不执行真实文件写入
    async fn write_binary_if(
        &self,
        _: BinaryConditionalWrite,
        data: BinaryData,
        _: SystemContext,
        _: Capabilities,
    ) -> Result<bool> {
        if !self.blocking.load(Ordering::SeqCst) {
            return Ok(true);
        }
        let worker = self.worker.clone();
        tokio::task::spawn_blocking(move || {
            let result = worker.wait();
            drop(data);
            worker.released.notify_one();
            result?;
            Ok(true)
        })
        .await?
    }
}
