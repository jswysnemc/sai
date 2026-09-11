use base64::Engine;
use rusqlite::{Connection, DatabaseName};

/// 【数据库快照测试】【独立镜像】通过原生 SQLite 构造插件接口之外的有效或特殊数据库
/// @param sql 仅测试侧执行的固定建表和填充语句
/// @returns 完整独立数据库字节
pub fn snapshot(sql: &str) -> Vec<u8> {
    let connection = Connection::open_in_memory().unwrap();
    connection.execute_batch(sql).unwrap();
    connection.serialize(DatabaseName::Main).unwrap().to_vec()
}

/// 【数据库快照测试】【二进制传入】编码镜像以复用公开二进制构造入口
/// @param bytes 完整数据库字节
/// @returns 标准 Base64 文本
pub fn encoded(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}
