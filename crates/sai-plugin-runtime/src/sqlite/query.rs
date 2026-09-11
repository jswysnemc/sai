use super::{connection::Snapshot, Budget, Query, QueryResult, Record, MAX_VALUE_BYTES};
use anyhow::{bail, ensure, Result};
use rusqlite::{
    params_from_iter,
    types::{Value as SqlValue, ValueRef},
};
use serde_json::{Map, Number, Value};

/// 【数据库快照】【标识引用】调用方已经完成 ASCII 标识校验，双引号保留关键字列名
/// @param name 合法标识
/// @returns SQL 引用文本
pub(super) fn quoted(name: &str) -> String {
    format!("\"{name}\"")
}

/// 【数据库快照】【参数绑定】文本始终使用参数，布尔值按照 SQLite 整数保存
/// @param value 已验证的 JSON 标量
/// @returns SQLite 参数值
pub(super) fn parameter(value: &Value) -> SqlValue {
    match value {
        Value::Null => SqlValue::Null,
        Value::Bool(value) => SqlValue::Integer(i64::from(*value)),
        Value::Number(value) if value.is_i64() => SqlValue::Integer(value.as_i64().unwrap()),
        Value::Number(value) => SqlValue::Real(value.as_f64().unwrap()),
        Value::String(value) => SqlValue::Text(value.clone()),
        _ => unreachable!("validated SQLite scalar"),
    }
}

/// 【数据库快照】【等值过滤】使用 IS 参数同时覆盖普通值与 SQL null
/// @param filter 已验证字段映射
/// @returns 可选 WHERE 子句和独立参数
pub(super) fn predicate(filter: &Record) -> (String, Vec<SqlValue>) {
    if filter.is_empty() {
        return (String::new(), Vec::new());
    }
    (
        format!(
            " WHERE {}",
            filter
                .keys()
                .map(|key| format!("{} IS ?", quoted(key)))
                .collect::<Vec<_>>()
                .join(" AND ")
        ),
        filter.values().map(parameter).collect(),
    )
}

/// 【数据库快照】【有界查询】从普通表读取指定列，禁止隐式截断或返回部分失败结果
/// @param bytes 原数据库；query 为已验证请求；budget 为共用预算；output_bytes 为结果上限
/// @returns 列顺序与完整分页记录
pub(crate) fn query(
    bytes: &[u8],
    query: Query,
    budget: Budget,
    output_bytes: usize,
) -> Result<QueryResult> {
    // 1. 【数据库快照】【只读准备】验证普通表，只用已校验标识和参数组装固定查询
    let snapshot = Snapshot::open(Some(bytes), bytes.len(), true, budget.clone())?;
    snapshot.table(&query.table)?;
    let (filter, parameters) = predicate(&query.filter);
    let mut sql = format!(
        "SELECT {} FROM {} NOT INDEXED{filter}",
        query
            .columns
            .iter()
            .map(|name| quoted(name))
            .collect::<Vec<_>>()
            .join(","),
        quoted(&query.table)
    );
    if !query.order_by.is_empty() {
        sql.push_str(" ORDER BY ");
        sql.push_str(
            &query
                .order_by
                .iter()
                .map(|order| {
                    format!(
                        "{} {}",
                        quoted(&order.column),
                        if order.descending { "DESC" } else { "ASC" }
                    )
                })
                .collect::<Vec<_>>()
                .join(","),
        );
    }
    sql.push_str(&format!(" LIMIT {} OFFSET {}", query.limit, query.offset));
    // 2. 【数据库快照】【完整读取】逐行校验类型和 JSON 总量，超限不保留部分结果
    budget(1)?;
    let mut statement = snapshot.database.connection().prepare(&sql)?;
    let mut rows = statement.query(params_from_iter(parameters))?;
    let mut result = QueryResult {
        columns: query.columns,
        rows: Vec::new(),
    };
    let mut size = serde_json::to_vec(&result)?.len();
    ensure!(
        size <= output_bytes,
        "SQLite query result exceeds output limit"
    );
    while let Some(row) = rows.next()? {
        budget(1)?;
        let mut item = Map::new();
        for (index, column) in result.columns.iter().enumerate() {
            let value = match row.get_ref(index)? {
                ValueRef::Null => Value::Null,
                ValueRef::Integer(value) => Value::from(value),
                ValueRef::Real(value) => Value::Number(
                    Number::from_f64(value)
                        .ok_or_else(|| anyhow::anyhow!("SQLite result is not finite"))?,
                ),
                ValueRef::Text(bytes) => {
                    ensure!(
                        bytes.len() <= MAX_VALUE_BYTES,
                        "SQLite text result exceeds 256 KiB"
                    );
                    budget(bytes.len() as u64)?;
                    Value::String(std::str::from_utf8(bytes)?.into())
                }
                ValueRef::Blob(_) => bail!("SQLite blob columns are not supported"),
            };
            item.insert(column.clone(), value);
        }
        size = size
            .saturating_add(serde_json::to_vec(&item)?.len())
            .saturating_add(usize::from(!result.rows.is_empty()));
        ensure!(
            size <= output_bytes,
            "SQLite query result exceeds output limit"
        );
        result.rows.push(item);
    }
    budget(0)?;
    Ok(result)
}
