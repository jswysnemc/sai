use super::{
    connection::Snapshot,
    query::{parameter, predicate, quoted},
    Budget, Change, Kind, Record,
};
use crate::host::{BinaryData, BinaryReadBuffer};
use anyhow::Result;
use rusqlite::{params_from_iter, Connection};

/// 【数据库快照】【原子批次】只修改独立内存副本，全部成功后交付序列化缓冲
/// @param bytes 可选原数据库；changes 为已验证变更；output 为预留结果；budget 为共用预算
/// @returns 新快照，失败时原缓冲保持不变
pub(crate) fn apply(
    bytes: Option<&[u8]>,
    changes: Vec<Change>,
    mut output: BinaryReadBuffer,
    budget: Budget,
) -> Result<BinaryData> {
    // 1. 【数据库快照】【独立事务】只对完整内存副本执行变更，原输入没有可变引用
    let mut snapshot = Snapshot::open(bytes, output.max_bytes(), false, budget.clone())?;
    let transaction = snapshot.database.connection_mut().transaction()?;
    for change in changes {
        budget(1)?;
        execute(&transaction, change, &budget)?;
    }
    // 2. 【数据库快照】【提交检查】提交后重新验证结构，限制整批新增对象总数
    budget(0)?;
    transaction.commit()?;
    super::connection::validate_schema(snapshot.database.connection())?;
    // 3. 【数据库快照】【完整导出】分块复制到预留输出，任何错误都不返回部分镜像
    snapshot.export(&mut output)?;
    budget(0)?;
    Ok(output.finish())
}

/// 【数据库快照】【声明式执行】SQL 结构仅由已验证的标识和固定模板组成
/// @param connection 当前事务；change 为单项变更；budget 为工作预算
/// @returns 该项变更完成结果
fn execute(connection: &Connection, change: Change, budget: &Budget) -> Result<()> {
    match change {
        Change::CreateTable {
            name,
            columns,
            primary_key,
            auto_increment,
        } => {
            let definitions = columns
                .iter()
                .map(|column| {
                    let kind = match column.kind {
                        Kind::Integer => "INTEGER",
                        Kind::Real => "REAL",
                        Kind::Text => "TEXT",
                    };
                    let primary = primary_key
                        .as_ref()
                        .is_some_and(|key| column.name.eq_ignore_ascii_case(key));
                    format!(
                        "{} {kind}{}{}{}",
                        quoted(&column.name),
                        if primary { " PRIMARY KEY" } else { "" },
                        if primary && auto_increment {
                            " AUTOINCREMENT"
                        } else {
                            ""
                        },
                        if column.nullable { "" } else { " NOT NULL" }
                    )
                })
                .collect::<Vec<_>>();
            connection.execute(
                &format!(
                    "CREATE TABLE IF NOT EXISTS {} ({})",
                    quoted(&name),
                    definitions.join(",")
                ),
                [],
            )?;
            super::connection::check_table(connection, &name)?;
        }
        Change::CreateIndex {
            name,
            table,
            columns,
            unique,
        } => {
            super::connection::check_table(connection, &table)?;
            connection.execute(
                &format!(
                    "CREATE {}INDEX IF NOT EXISTS {} ON {} ({})",
                    if unique { "UNIQUE " } else { "" },
                    quoted(&name),
                    quoted(&table),
                    columns
                        .iter()
                        .map(|column| quoted(column))
                        .collect::<Vec<_>>()
                        .join(",")
                ),
                [],
            )?;
        }
        Change::Insert { table, rows } => write_rows(connection, &table, None, rows, budget)?,
        Change::Upsert { table, key, rows } => {
            write_rows(connection, &table, Some(&key), rows, budget)?
        }
        Change::Delete { table, filter } => {
            super::connection::check_table(connection, &table)?;
            let (clause, parameters) = predicate(&filter);
            connection.execute(
                &format!("DELETE FROM {}{clause}", quoted(&table)),
                params_from_iter(parameters),
            )?;
        }
    }
    budget(0)
}

/// 【数据库快照】【行写入】逐行参数绑定，冲突更新仅修改本次明确提供的字段
/// @param connection 当前事务；table 为表名；key 为可选冲突键；rows 为记录；budget 为工作预算
/// @returns 全部行写入结果，任一失败由外层事务回滚
fn write_rows(
    connection: &Connection,
    table: &str,
    key: Option<&str>,
    rows: Vec<Record>,
    budget: &Budget,
) -> Result<()> {
    super::connection::check_table(connection, table)?;
    for row in rows {
        budget(1)?;
        let columns = row.keys().map(|name| quoted(name)).collect::<Vec<_>>();
        let mut sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            quoted(table),
            columns.join(","),
            vec!["?"; row.len()].join(",")
        );
        if let Some(key) = key {
            let updates = row
                .keys()
                .filter(|column| !column.eq_ignore_ascii_case(key))
                .map(|column| format!("{}=excluded.{}", quoted(column), quoted(column)))
                .collect::<Vec<_>>();
            sql.push_str(&format!(" ON CONFLICT ({}) DO ", quoted(key)));
            if updates.is_empty() {
                sql.push_str("NOTHING");
            } else {
                sql.push_str("UPDATE SET ");
                sql.push_str(&updates.join(","));
            }
        }
        connection.execute(&sql, params_from_iter(row.values().map(parameter)))?;
    }
    Ok(())
}
