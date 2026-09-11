use super::{Allocation, BinaryBudget, BinaryData};
use anyhow::{bail, Context, Result};
use std::sync::{atomic::Ordering, Arc};

/// 【插件二进制】【读取缓冲】独占预留额度，宿主必须把本对象移入实际读取线程
pub struct BinaryReadBuffer {
    allocation: Allocation,
}

impl BinaryBudget {
    /// 【插件二进制】【读取预留】开始宿主读取前预留上限，尚未收到字节也不能重复分配这部分额度
    /// @param max_bytes 本次完整文件读取的非零上限
    /// @returns 不可克隆的有界缓冲，销毁时自动归还未完成读取的全部额度
    pub(crate) fn reserve(self: &Arc<Self>, max_bytes: usize) -> Result<BinaryReadBuffer> {
        if max_bytes == 0 {
            bail!("plugin binary read byte limit must be positive");
        }
        self.charge(max_bytes)?;
        Ok(BinaryReadBuffer {
            allocation: Allocation {
                bytes: Vec::new(),
                charged: max_bytes,
                budget: self.clone(),
            },
        })
    }
}

impl BinaryReadBuffer {
    /// 【插件二进制】【读取上限】返回本次已预留的最大字节数
    /// @returns 完整文件的有效大小上限
    pub fn max_bytes(&self) -> usize {
        self.allocation.charged
    }

    /// 【插件二进制】【剩余空间】返回尚未填入数据的预留空间
    /// @returns 可以继续追加的字节数
    pub fn remaining(&self) -> usize {
        self.max_bytes() - self.allocation.bytes.len()
    }

    /// 【插件二进制】【有界追加】先检查预留额度再分配和复制，拒绝静默截断
    /// @param bytes 从文件读取的下一段原始字节
    /// @returns 追加成功时成功，超限或分配失败时原缓冲保持有效
    pub fn extend_from_slice(&mut self, bytes: &[u8]) -> Result<()> {
        if bytes.len() > self.remaining() {
            bail!("plugin binary file exceeds size limit");
        }
        self.allocation
            .bytes
            .try_reserve_exact(bytes.len())
            .context("allocate plugin binary read buffer")?;
        self.allocation.bytes.extend_from_slice(bytes);
        Ok(())
    }

    /// 【插件二进制】【读取完成】将完整读取结果转换为不可变数据，并归还未用预留额度
    /// @returns 按实际字节数计费的缓冲；宿主必须先确认完整读取成功
    pub fn finish(mut self) -> BinaryData {
        let used = self.allocation.bytes.len();
        let unused = self.allocation.charged - used;
        self.allocation.charged = used;
        self.allocation
            .budget
            .retained
            .fetch_sub(unused, Ordering::AcqRel);
        BinaryData(Arc::new(self.allocation))
    }
}
