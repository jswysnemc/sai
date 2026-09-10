use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

pub(crate) type BinarySlot = std::sync::Mutex<Option<BinaryData>>;

/// 【插件二进制】【网络结果】响应正文保留原始字节，不经过文本解码或模型输出。
pub struct BinaryResponse {
    pub status: u16,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

/// 【插件二进制】【输出结果】写入后只返回文件位置和字节数。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BinaryFile {
    pub path: String,
    pub bytes: usize,
}

/// 【插件二进制】【绘制结果】宿主返回实际使用的路径，不返回图像内容。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DisplayedImage {
    pub path: String,
}

/// 【插件二进制】【保留预算】同一个 VM 的所有缓冲和仍在执行的文件 I/O 共用上限。
pub(crate) struct BinaryBudget {
    limit: usize,
    retained: AtomicUsize,
}

struct Allocation {
    bytes: Vec<u8>,
    budget: Arc<BinaryBudget>,
}

/// 【插件二进制】【不可变缓冲】克隆只增加引用，最后一个持有者释放时归还预算。
#[derive(Clone)]
pub struct BinaryData(Arc<Allocation>);

impl BinaryData {
    /// 【插件二进制】【字节借用】只读访问缓冲，宿主不得将其当作可执行内容。
    /// @returns 当前不可变字节切片
    pub fn bytes(&self) -> &[u8] {
        &self.0.bytes
    }
}

impl BinaryBudget {
    /// 【插件二进制】【剩余额度】在下载或解码分配前限制新缓冲大小。
    /// @returns 扣除仍被持有的缓冲后可用的字节数
    pub(crate) fn available(&self) -> usize {
        self.limit
            .saturating_sub(self.retained.load(Ordering::Acquire))
    }

    /// 【插件二进制】【预算创建】为一个 VM 创建保留字节预算。
    /// @param limit 本 VM 最多保留的原始字节数
    /// @returns 可跨异步调用共享的预算
    pub(crate) fn new(limit: usize) -> Arc<Self> {
        Arc::new(Self {
            limit,
            retained: AtomicUsize::new(0),
        })
    }

    /// 【插件二进制】【缓冲登记】登记已经有界读取的数据，防止多个缓冲绕过总上限。
    /// @param bytes 待持有的字节
    /// @returns 带预算租约的不可变缓冲；不足时不保留数据
    pub(crate) fn retain(self: &Arc<Self>, bytes: Vec<u8>) -> Result<BinaryData> {
        let size = bytes.len();
        if self
            .retained
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                used.checked_add(size).filter(|next| *next <= self.limit)
            })
            .is_err()
        {
            bail!("plugin retained binary buffers exceed size limit");
        }
        Ok(BinaryData(Arc::new(Allocation {
            bytes,
            budget: self.clone(),
        })))
    }
}

impl Drop for Allocation {
    /// 【插件二进制】【预算回收】最后一个缓冲持有者释放后归还所有字节。
    /// @returns 无；阻塞文件写入未结束时仍持有该租约
    fn drop(&mut self) {
        self.budget
            .retained
            .fetch_sub(self.bytes.len(), Ordering::AcqRel);
    }
}
