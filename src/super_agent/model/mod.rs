mod entities;
mod enums;

pub use entities::*;
pub use enums::*;

/// 返回当前 Unix 毫秒时间戳。
///
/// 参数:
/// - 无
///
/// 返回:
/// - Unix 毫秒时间戳
pub fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}

/// 生成带前缀的稳定唯一标识。
///
/// 参数:
/// - `prefix`: 标识前缀
///
/// 返回:
/// - 唯一标识字符串
pub fn new_id(prefix: &str) -> String {
    let raw = uuid::Uuid::new_v4().simple().to_string();
    format!("{prefix}_{}", &raw[..12])
}
