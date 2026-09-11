use anyhow::{bail, Result};
use rusqlite::Connection;
use sai_sqlite_buffer::MemoryDatabase;

/// 【SQLite 内存测试】【有效样本】创建已提交数据的独立固定容量数据库
/// @param capacity 数据库总容量
/// @returns 包含普通表及一条文字记录的数据库
fn seeded(capacity: usize) -> MemoryDatabase {
    let database = MemoryDatabase::new(
        Connection::open_in_memory().unwrap(),
        None,
        capacity,
        false,
        |_| Ok(()),
    )
    .unwrap();
    database.connection().execute_batch("CREATE TABLE notes(id INTEGER PRIMARY KEY, text TEXT); INSERT INTO notes VALUES(1,'original');").unwrap();
    database
}

/// 【SQLite 内存测试】【所有权接续】原字节可释放，反序列化连接继续持有自己的完整副本
/// @returns 无；只读标志由 SQLite 执行，不能修改副本
#[test]
fn snapshots_own_their_bytes_and_enforce_readonly() {
    let mut original = seeded(65536);
    let mut bytes = Vec::new();
    original
        .copy_to(|chunk| {
            bytes.extend_from_slice(chunk);
            Ok(())
        })
        .unwrap();
    let restored = MemoryDatabase::new(
        Connection::open_in_memory().unwrap(),
        Some(&bytes),
        bytes.len(),
        true,
        |_| Ok(()),
    )
    .unwrap();
    bytes.fill(0);
    drop(original);
    let text: String = restored
        .connection()
        .query_row("SELECT text FROM notes WHERE id=1", [], |row| row.get(0))
        .unwrap();
    assert_eq!(text, "original");
    assert!(restored
        .connection()
        .execute("UPDATE notes SET text='changed'", [])
        .is_err());
}

/// 【SQLite 内存测试】【容量不扩大】增长超过原固定分配时失败，已有提交仍可导出
/// @returns 无；不会使用 SQLite 自动扩容绕过上限
#[test]
fn database_growth_is_bounded_by_the_fixed_buffer() {
    let mut database = seeded(8192);
    assert!(database
        .connection()
        .execute("INSERT INTO notes(text) VALUES(zeroblob(65536))", [])
        .is_err());
    let count: i64 = database
        .connection()
        .query_row("SELECT count(*) FROM notes", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 1);
    let mut bytes = 0;
    database
        .copy_to(|chunk| {
            bytes += chunk.len();
            Ok(())
        })
        .unwrap();
    assert!(bytes <= 8192);
}

/// 【SQLite 内存测试】【中断回收】构造和导出检查失败后不损坏其他数据库或原连接
/// @returns 无；再次导出得到相同完整内容
#[test]
fn failed_allocation_checks_and_export_callbacks_preserve_ownership() {
    for _ in 0..16 {
        let mut blocks = 0;
        let result = MemoryDatabase::new(
            Connection::open_in_memory().unwrap(),
            None,
            131072,
            false,
            |_| {
                blocks += 1;
                if blocks == 2 {
                    bail!("stop initialization");
                }
                Ok(())
            },
        );
        assert!(result.is_err());
        assert_eq!(blocks, 2);
    }
    let mut database = seeded(65536);
    assert!(database
        .copy_to(|_| -> Result<()> { bail!("stop export") })
        .is_err());
    let mut first = Vec::new();
    database
        .copy_to(|chunk| {
            first.extend_from_slice(chunk);
            Ok(())
        })
        .unwrap();
    let mut second = Vec::new();
    database
        .copy_to(|chunk| {
            second.extend_from_slice(chunk);
            Ok(())
        })
        .unwrap();
    assert_eq!(first, second);
}

/// 【SQLite 内存测试】【容量输入】非法容量及超过容量的来源在分配前失败
/// @returns 无；检查函数没有机会读取越界输入
#[test]
fn invalid_capacity_is_rejected_before_allocating() {
    for capacity in [0, 64 * 1024 * 1024 + 1] {
        assert!(MemoryDatabase::new(
            Connection::open_in_memory().unwrap(),
            None,
            capacity,
            false,
            |_| panic!("unexpected allocation")
        )
        .is_err());
    }
    assert!(MemoryDatabase::new(
        Connection::open_in_memory().unwrap(),
        Some(&[0; 32]),
        16,
        false,
        |_| panic!("unexpected allocation")
    )
    .is_err());
}
