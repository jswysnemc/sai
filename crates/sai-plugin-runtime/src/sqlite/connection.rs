use super::{Budget, MAX_DATABASE_BYTES, MAX_REQUEST_BYTES};
use crate::host::BinaryReadBuffer;
use anyhow::{bail, ensure, Result};
use rusqlite::{
    config::DbConfig,
    hooks::{AuthAction, AuthContext, Authorization},
    limits::Limit,
    Connection,
};
use sai_sqlite_buffer::MemoryDatabase;

/// 【数据库快照】【固定内存】SQLite 持有固定容量缓冲，不取得文件路径或扩容权限
pub(super) struct Snapshot {
    pub database: MemoryDatabase,
    budget: Budget,
}

impl Snapshot {
    /// 【数据库快照】【内存连接】复制完整镜像并限制页数、字段、执行指令和外部访问
    /// @param bytes 可选数据库镜像；capacity 为固定容量；readonly 为查询模式；budget 为共用预算
    /// @returns 仅能处理结构化请求的内存连接
    pub fn open(
        bytes: Option<&[u8]>,
        capacity: usize,
        readonly: bool,
        budget: Budget,
    ) -> Result<Self> {
        ensure!(
            capacity > 0 && capacity <= MAX_DATABASE_BYTES,
            "SQLite snapshot capacity is invalid"
        );
        if let Some(bytes) = bytes {
            validate_image(bytes, capacity)?;
        }
        budget(0)?;
        let connection = Connection::open_in_memory()?;
        for (option, value) in [
            (DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true),
            (DbConfig::SQLITE_DBCONFIG_TRUSTED_SCHEMA, false),
            (DbConfig::SQLITE_DBCONFIG_ENABLE_VIEW, false),
            (DbConfig::SQLITE_DBCONFIG_DQS_DDL, false),
            (DbConfig::SQLITE_DBCONFIG_DQS_DML, false),
        ] {
            connection.set_db_config(option, value)?;
        }
        for (limit, value) in [
            (Limit::SQLITE_LIMIT_LENGTH, MAX_REQUEST_BYTES as i32),
            (Limit::SQLITE_LIMIT_SQL_LENGTH, 65536),
            (Limit::SQLITE_LIMIT_COLUMN, 64),
            (Limit::SQLITE_LIMIT_EXPR_DEPTH, 64),
            (Limit::SQLITE_LIMIT_COMPOUND_SELECT, 1),
            (Limit::SQLITE_LIMIT_VDBE_OP, 25000),
            (Limit::SQLITE_LIMIT_ATTACHED, 0),
            (Limit::SQLITE_LIMIT_VARIABLE_NUMBER, 128),
            (Limit::SQLITE_LIMIT_TRIGGER_DEPTH, 0),
        ] {
            connection.set_limit(limit, value);
        }
        let progress = budget.clone();
        connection.progress_handler(512, Some(move || progress(512).is_err()));
        // 1. 【数据库快照】【固定分配】先建立受预算保护的副本，每块复制或清零均检查取消
        let database = MemoryDatabase::new(connection, bytes, capacity, readonly, |units| {
            budget(units as u64)
        })?;
        let connection = database.connection();
        connection.pragma_update(None, "temp_store", "MEMORY")?;
        connection.pragma_update(None, "cache_size", -256)?;
        let page_size: usize =
            connection.pragma_query_value(None, "page_size", |row| row.get(0))?;
        let page_count: usize =
            connection.pragma_query_value(None, "page_count", |row| row.get(0))?;
        ensure!(
            page_size > 0 && page_count <= capacity / page_size,
            "SQLite snapshot page count exceeds capacity"
        );
        if !readonly {
            connection.pragma_update(None, "max_page_count", capacity / page_size)?;
        }
        // 2. 【数据库快照】【禁止外部行为】运行时只生成固定模板，授权器再次阻止路径、虚表、函数及附属程序
        connection.authorizer(Some(move |context: AuthContext<'_>| {
            authorize(context, readonly)
        }));
        validate_schema(&connection)?;
        budget(0)?;
        Ok(Self { database, budget })
    }

    /// 【数据库快照】【普通表检查】拒绝视图、虚表、生成列和外键隐式更新
    /// @param name 已校验的表名
    /// @returns 普通表可以查询或修改时成功
    pub fn table(&self, name: &str) -> Result<()> {
        check_table(self.database.connection(), name)
    }

    /// 【数据库快照】【无额外分配导出】借用固定数据库镜像，分块复制进预留结果
    /// @param output 预留的有界结果缓冲
    /// @returns 完整复制结果，不交付部分镜像
    pub fn export(&mut self, output: &mut BinaryReadBuffer) -> Result<()> {
        let budget = self.budget.clone();
        self.database.copy_to(|chunk| {
            budget(chunk.len() as u64)?;
            output.extend_from_slice(chunk)
        })
    }
}

/// 【数据库快照】【镜像校验】拒绝空、损坏、超限及依赖独立 WAL 文件的输入
/// @param bytes 完整输入；capacity 为固定容量
/// @returns 可作为独立回滚日志格式镜像使用时成功
fn validate_image(bytes: &[u8], capacity: usize) -> Result<()> {
    ensure!(
        bytes.len() >= 100 && bytes.len() <= capacity && bytes.starts_with(b"SQLite format 3\0"),
        "invalid SQLite snapshot header or size"
    );
    ensure!(
        bytes[18] == 1 && bytes[19] == 1,
        "SQLite WAL snapshots require a checkpoint and rollback-journal format before import"
    );
    let size = u16::from_be_bytes([bytes[16], bytes[17]]);
    let size = if size == 1 { 65536 } else { usize::from(size) };
    ensure!(
        (512..=65536).contains(&size) && size.is_power_of_two() && bytes.len() % size == 0,
        "invalid SQLite snapshot page size"
    );
    Ok(())
}

/// 【数据库快照】【授权复核】只允许固定模板所需操作和三个只读结构查询
/// @param context SQLite 授权事实；readonly 为当前调用模式
/// @returns 允许或拒绝，不隐藏非法访问
fn authorize(context: AuthContext<'_>, readonly: bool) -> Authorization {
    if context.accessor.is_some()
        || context
            .database_name
            .is_some_and(|database| database != "main")
    {
        return Authorization::Deny;
    }
    let allowed = match context.action {
        AuthAction::Select | AuthAction::Read { .. } => true,
        AuthAction::Pragma {
            pragma_name,
            pragma_value,
        } => match pragma_name {
            "table_list" => pragma_value.is_none(),
            "table_xinfo" | "foreign_key_list" => {
                pragma_value.is_some_and(|name| super::validation::identifier(name).is_ok())
            }
            _ => false,
        },
        AuthAction::CreateTable { .. }
        | AuthAction::CreateIndex { .. }
        | AuthAction::Insert { .. }
        | AuthAction::Update { .. }
        | AuthAction::Delete { .. }
        | AuthAction::Transaction { .. }
        | AuthAction::Savepoint { .. }
        | AuthAction::Reindex { .. } => !readonly,
        _ => false,
    };
    if allowed {
        Authorization::Allow
    } else {
        Authorization::Deny
    }
}

/// 【数据库快照】【结构边界】限制结构对象数，并拒绝任何视图、触发器和虚拟表
/// @param connection 已受限的内存连接
/// @returns 只包含普通表和索引的数据库
pub(super) fn validate_schema(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("SELECT type FROM sqlite_schema LIMIT 129")?;
    let mut rows = statement.query([])?;
    let mut count = 0;
    while let Some(row) = rows.next()? {
        count += 1;
        let kind: String = row.get(0)?;
        ensure!(matches!(kind.as_str(),"table"|"index") && count<=128,"SQLite snapshot supports at most 128 ordinary tables and indexes without views or triggers");
    }
    let mut statement = connection.prepare("PRAGMA main.table_list")?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let kind: String = row.get(2)?;
        ensure!(kind == "table", "SQLite virtual tables are not supported");
    }
    Ok(())
}

/// 【数据库快照】【表字段边界】不执行附带的生成表达式或外键级联
/// @param connection 受限连接；name 为已校验的普通表名
/// @returns 表存在且只含普通字段时成功
pub(super) fn check_table(connection: &Connection, name: &str) -> Result<()> {
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name=?1 COLLATE NOCASE)",
        [name],
        |row| row.get(0),
    )?;
    ensure!(exists, "SQLite table does not exist: {name}");
    let mut statement = connection.prepare(&format!(
        "PRAGMA main.table_xinfo({})",
        super::query::quoted(name)
    ))?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let hidden: i64 = row.get(6)?;
        ensure!(hidden == 0, "SQLite generated columns are not supported");
    }
    let mut statement = connection.prepare(&format!(
        "PRAGMA main.foreign_key_list({})",
        super::query::quoted(name)
    ))?;
    if statement.query([])?.next()?.is_some() {
        bail!("SQLite foreign keys are not supported");
    }
    Ok(())
}
