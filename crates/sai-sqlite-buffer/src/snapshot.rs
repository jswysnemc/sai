use crate::allocation::Allocation;
use anyhow::{ensure, Result};
use rusqlite::{ffi, Connection};

/// 【SQLite 内存】【固定镜像】连接拥有固定容量镜像，不允许 SQLite 重新分配缓冲
pub struct MemoryDatabase {
    connection: Connection,
    capacity: usize,
}

impl MemoryDatabase {
    /// 【SQLite 内存】【反序列化】接管独立缓冲，只替换传入连接的 main 内存数据库
    /// @param connection 独占连接；source 为镜像；capacity 为上限；readonly 为只读；check 为预算检查
    /// @returns 固定内存数据库，失败不保留来源引用或分配
    pub fn new(
        connection: Connection,
        source: Option<&[u8]>,
        capacity: usize,
        readonly: bool,
        check: impl FnMut(usize) -> Result<()>,
    ) -> Result<Self> {
        let source = source.unwrap_or_default();
        ensure!(
            capacity > 0 && capacity <= 64 * 1024 * 1024 && source.len() <= capacity,
            "invalid fixed SQLite buffer capacity"
        );
        let pointer = Allocation::new(source, capacity, check)?.into_raw();
        let flags = ffi::SQLITE_DESERIALIZE_FREEONCLOSE
            | if readonly {
                ffi::SQLITE_DESERIALIZE_READONLY
            } else {
                0
            };
        // 1. 【SQLite 内存】【接管安全】连接唯一持有句柄；SQLite 无论成功失败都会接管并最终释放 pointer
        let code = unsafe {
            ffi::sqlite3_deserialize(
                connection.handle(),
                c"main".as_ptr(),
                pointer,
                source.len() as i64,
                capacity as i64,
                flags,
            )
        };
        ensure!(
            code == ffi::SQLITE_OK,
            "SQLite snapshot deserialization failed: {code}"
        );
        Ok(Self {
            connection,
            capacity,
        })
    }

    /// 【SQLite 内存】【连接借用】借用句柄进行受控查询，借用期间不能导出镜像
    /// @returns 生命周期绑定到本对象的连接
    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    /// 【SQLite 内存】【事务借用】独占借用连接以执行事务
    /// @returns 生命周期绑定到本对象的可变连接
    pub fn connection_mut(&mut self) -> &mut Connection {
        &mut self.connection
    }

    /// 【SQLite 内存】【镜像复制】独占借用数据库，防止复制期间通过安全 Rust 改写连接
    /// @param append 接收每块临时字节的函数，不能保留借用
    /// @returns 完整镜像复制结果，不为 NOCOPY 失败分配额外副本
    pub fn copy_to(&mut self, mut append: impl FnMut(&[u8]) -> Result<()>) -> Result<()> {
        let mut size = 0i64;
        // 1. 【SQLite 内存】【只读借用安全】连接存活且由本方法独占，NOCOPY 返回的内存不会重分配
        let pointer = unsafe {
            ffi::sqlite3_serialize(
                self.connection.handle(),
                c"main".as_ptr(),
                &mut size,
                ffi::SQLITE_SERIALIZE_NOCOPY,
            )
        };
        ensure!(
            !pointer.is_null() && size > 0 && size as u64 <= self.capacity as u64,
            "SQLite snapshot cannot be serialized within capacity"
        );
        // 2. 【SQLite 内存】【长度安全】SQLite 确认有效长度，结果同时受固定容量限制
        let bytes = unsafe { std::slice::from_raw_parts(pointer, size as usize) };
        for chunk in bytes.chunks(32768) {
            append(chunk)?;
        }
        Ok(())
    }
}
