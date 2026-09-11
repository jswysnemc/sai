use super::{Change, Kind, Query, Record, MAX_VALUE_BYTES};
use anyhow::{bail, ensure, Result};
use serde_json::Value;
use std::collections::BTreeSet;

/// 【数据库快照】【标识校验】只接收有界 ASCII 标识，保留内部 sqlite 命名空间
/// @param name 表名、索引名或列名
/// @returns 合法时成功，不把标识解释为路径或表达式
pub(super) fn identifier(name: &str) -> Result<()> {
    let mut bytes = name.bytes();
    ensure!(
        !name.is_empty()
            && name.len() <= 64
            && bytes.next().is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
            && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
            && !name.to_ascii_lowercase().starts_with("sqlite_"),
        "SQLite identifiers must contain 1-64 ASCII letters, digits or underscores without the sqlite_ prefix"
    );
    Ok(())
}

/// 【数据库快照】【列集合】拒绝过多、重复或大小写歧义列名
/// @param names 待校验列名
/// @returns 非空且不超过三十二列时成功
fn names<'a>(names: impl Iterator<Item = &'a str>) -> Result<()> {
    let mut seen = BTreeSet::new();
    for name in names {
        identifier(name)?;
        ensure!(
            seen.insert(name.to_ascii_lowercase()),
            "duplicate SQLite column"
        );
        ensure!(seen.len() <= 32, "SQLite request exceeds 32 columns");
    }
    ensure!(!seen.is_empty(), "SQLite columns must not be empty");
    Ok(())
}

/// 【数据库快照】【标量边界】禁止嵌套对象、数组及无法精确表示的无符号整数
/// @param value 参数值
/// @returns SQLite 可绑定的有界 JSON 标量
pub(super) fn scalar(value: &Value) -> Result<()> {
    match value {
        Value::Null | Value::Bool(_) => {}
        Value::String(text) if text.len() <= MAX_VALUE_BYTES => {}
        Value::Number(number) => {
            ensure!(
                number.as_u64().is_none_or(|value| value <= i64::MAX as u64),
                "SQLite integer exceeds signed 64-bit range"
            );
        }
        _ => bail!("SQLite values must be scalars within 256 KiB"),
    }
    Ok(())
}

/// 【数据库快照】【记录校验】空过滤器可匹配整表，写入行必须包含字段
/// @param record 记录或过滤器；empty 为是否允许空映射
/// @returns 所有字段与值合法时成功
fn record(record: &Record, empty: bool) -> Result<()> {
    if record.is_empty() && empty {
        return Ok(());
    }
    names(record.keys().map(String::as_str))?;
    for value in record.values() {
        scalar(value)?;
    }
    Ok(())
}

/// 【数据库快照】【查询边界】限制投影、过滤、排序、行数和偏移量
/// @param query 完整结构化查询
/// @returns 查询可交给数据库工作线程时成功
pub(crate) fn validate_query(query: &Query) -> Result<()> {
    identifier(&query.table)?;
    names(query.columns.iter().map(String::as_str))?;
    record(&query.filter, true)?;
    ensure!(
        (1..=512).contains(&query.limit),
        "SQLite query limit must be within 1-512"
    );
    ensure!(
        query.offset <= 1_000_000,
        "SQLite query offset exceeds 1000000"
    );
    if !query.order_by.is_empty() {
        names(query.order_by.iter().map(|order| order.column.as_str()))?;
    }
    Ok(())
}

/// 【数据库快照】【变更边界】完整验证批次后才分配数据库或执行任何操作
/// @param changes 声明式事务列表
/// @returns 不超过六十四项变更及五百一十二行写入时成功
pub(crate) fn validate_changes(changes: &[Change]) -> Result<()> {
    ensure!(
        !changes.is_empty() && changes.len() <= 64,
        "SQLite batch must contain 1-64 changes"
    );
    let mut total_rows = 0usize;
    for change in changes {
        match change {
            Change::CreateTable {
                name,
                columns,
                primary_key,
                auto_increment,
            } => {
                identifier(name)?;
                names(columns.iter().map(|column| column.name.as_str()))?;
                if let Some(key) = primary_key {
                    identifier(key)?;
                    let column = columns
                        .iter()
                        .find(|column| column.name.eq_ignore_ascii_case(key));
                    ensure!(
                        column.is_some(),
                        "SQLite primary key must name a declared column"
                    );
                    ensure!(
                        !auto_increment || column.unwrap().kind == Kind::Integer,
                        "SQLite autoincrement requires an integer primary key"
                    );
                } else {
                    ensure!(
                        !auto_increment,
                        "SQLite autoincrement requires a primary key"
                    );
                }
            }
            Change::CreateIndex {
                name,
                table,
                columns,
                ..
            } => {
                identifier(name)?;
                identifier(table)?;
                names(columns.iter().map(String::as_str))?;
            }
            Change::Insert { table, rows } | Change::Upsert { table, rows, .. } => {
                identifier(table)?;
                ensure!(!rows.is_empty(), "SQLite rows must not be empty");
                total_rows = total_rows.saturating_add(rows.len());
                ensure!(total_rows <= 512, "SQLite batch exceeds 512 rows");
                for row in rows {
                    record(row, false)?;
                    if let Change::Upsert { key, .. } = change {
                        identifier(key)?;
                        ensure!(
                            row.iter()
                                .any(|(column, value)| column.eq_ignore_ascii_case(key)
                                    && !value.is_null()),
                            "SQLite upsert requires a non-null key in every row"
                        );
                    }
                }
            }
            Change::Delete { table, filter } => {
                identifier(table)?;
                record(filter, true)?;
            }
        }
    }
    Ok(())
}
