use super::model::{Turn, TurnStatus};
use anyhow::Result;
use rusqlite::{Connection, Row};

#[cfg(test)]
thread_local! {
    static DECODED_TURNS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// 【会话历史】【读取计数】读取当前测试线程实际解码的轮次数量。
/// @returns 累计解码行数；无参数
#[cfg(test)]
pub(super) fn decoded_turn_rows() -> usize {
    DECODED_TURNS.with(std::cell::Cell::get)
}

/// 用固定 SQL 加载轮次。
///
/// 参数:
/// - `conn`: 数据库连接
/// - `sql`: 查询语句
/// - `params`: 查询参数
///
/// 返回:
/// - 轮次列表
pub(super) fn load_turns_with_sql<P>(conn: &Connection, sql: &str, params: P) -> Result<Vec<Turn>>
where
    P: rusqlite::Params,
{
    let mut stmt = conn.prepare(sql)?;
    let turns = stmt
        .query_map(params, map_turn)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(turns)
}

/// 从查询行恢复轮次。
///
/// 参数:
/// - `row`: 查询行
///
/// 返回:
/// - 轮次
pub(super) fn map_turn(row: &Row<'_>) -> rusqlite::Result<Turn> {
    #[cfg(test)]
    DECODED_TURNS.with(|count| count.set(count.get() + 1));
    let image_urls_json: String = row.get(3)?;
    let user_image_urls = serde_json::from_str(&image_urls_json).unwrap_or_default();
    let tool_reports_json: String = row.get(9)?;
    let tool_reports = serde_json::from_str(&tool_reports_json).unwrap_or_default();
    let status: String = row.get(8)?;
    Ok(Turn {
        turn_id: row.get(0)?,
        seq: row.get(1)?,
        user_content: row.get(2)?,
        user_image_urls,
        user_timestamp: row.get(4)?,
        assistant_content: row.get(5)?,
        assistant_reasoning: row.get(6)?,
        assistant_timestamp: row.get(7)?,
        status: TurnStatus::from_str(&status),
        tool_reports,
        duration_ms: row.get::<_, i64>(10).unwrap_or(0).max(0) as u64,
        parent_turn_id: row.get::<_, Option<String>>(11).unwrap_or(None),
        model: row
            .get::<_, Option<String>>(12)
            .unwrap_or(None)
            .filter(|model| !model.trim().is_empty()),
        error: row
            .get::<_, Option<String>>(13)
            .unwrap_or(None)
            .filter(|error| !error.trim().is_empty()),
    })
}
