use super::{buffer::Buffer, sqlite_request::decode, BinaryServices};
use crate::{
    host::BinaryData,
    sqlite::{
        self, Change, Options, Query, MAX_DATABASE_BYTES, MAX_REQUEST_BYTES, WORKSPACE_OVERHEAD,
    },
};
use mlua::{Lua, MultiValue, Table, Value};
use std::{sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;

#[cfg(test)]
#[path = "sqlite_worker_tests.rs"]
mod worker_tests;

/// 【数据库快照】【Lua 入口】数据库只来自本回调的二进制句柄，文件能力仍由已有入口授权
/// @param lua 虚拟机；api 为 sai 表；services 为可信调用及二进制预算
/// @returns 查询和批量变更函数的安装结果
pub(super) fn install(lua: &Lua, api: &Table, services: Arc<BinaryServices>) -> mlua::Result<()> {
    let sqlite = lua.create_table()?;
    let query_services = services.clone();
    sqlite.set(
        "query",
        lua.create_async_function(move |lua, mut arguments: MultiValue| {
            let services = query_services.clone();
            async move {
                // 1. 【数据库快照】【查询输入】先检查回调、句柄归属和完整结构化请求
                let generation = services.generation()?;
                if arguments.len() != 2 {
                    return Err(mlua::Error::runtime(
                        "SQLite query requires a snapshot and query object",
                    ));
                }
                let data = input(arguments.pop_front().unwrap(), &services, false)?.unwrap();
                let query: Query = decode(&lua, arguments.pop_front().unwrap(), &services)?;
                sqlite::validate_query(&query).map_err(super::super::system::error)?;
                // 2. 【数据库快照】【查询计算】预留副本与结果空间，再把额度随工作移交线程
                let output_bytes = services.limits.output_bytes.min(MAX_REQUEST_BYTES);
                let working = data
                    .bytes()
                    .len()
                    .checked_mul(2)
                    .and_then(|size| size.checked_add(WORKSPACE_OVERHEAD + output_bytes))
                    .ok_or_else(|| mlua::Error::runtime("SQLite working budget overflow"))?;
                let reservation = services
                    .budget
                    .reserve(working)
                    .map_err(super::super::system::error)?;
                services.charge()?;
                let result = work(&lua, services.limits.binary_timeout_ms, move |budget| {
                    let _reservation = reservation;
                    sqlite::query(data.bytes(), query, budget, output_bytes)
                })
                .await?;
                // 3. 【数据库快照】【查询交付】只在同一回调仍然有效时转换完整结果
                services.check(generation)?;
                super::super::system::result_value(&lua, &result, output_bytes)
            }
        })?,
    )?;
    sqlite.set(
        "apply",
        lua.create_async_function(move |lua, mut arguments: MultiValue| {
            let services = services.clone();
            async move {
                // 1. 【数据库快照】【变更输入】完整校验批次和选项，不先修改任何数据库
                let generation = services.generation()?;
                if !(2..=3).contains(&arguments.len()) {
                    return Err(mlua::Error::runtime(
                        "SQLite apply requires a snapshot, changes and optional limits",
                    ));
                }
                let data = input(arguments.pop_front().unwrap(), &services, true)?;
                let changes: Vec<Change> = decode(&lua, arguments.pop_front().unwrap(), &services)?;
                let options: Options = match arguments.pop_front().unwrap_or(Value::Nil) {
                    Value::Nil => Options::default(),
                    value => decode(&lua, value, &services)?,
                };
                sqlite::validate_changes(&changes).map_err(super::super::system::error)?;
                // 2. 【数据库快照】【容量收窄】工作副本和输出共同受剩余二进制额度限制
                let available = services
                    .budget
                    .available()
                    .saturating_sub(WORKSPACE_OVERHEAD)
                    / 3;
                let default_capacity = data.as_ref().map_or(1024 * 1024, |data| {
                    data.bytes().len().saturating_mul(2).max(1024 * 1024)
                });
                let capacity = options
                    .max_bytes
                    .unwrap_or(default_capacity)
                    .min(available)
                    .min(MAX_DATABASE_BYTES);
                if options.max_bytes == Some(0)
                    || options.timeout_ms == Some(0)
                    || capacity < 8192
                    || data
                        .as_ref()
                        .is_some_and(|data| data.bytes().len() > capacity)
                {
                    return Err(mlua::Error::runtime(
                        "SQLite capacity or timeout is invalid or exceeds available budget",
                    ));
                }
                let timeout_ms = options
                    .timeout_ms
                    .unwrap_or(services.limits.binary_timeout_ms)
                    .min(services.limits.binary_timeout_ms);
                let working = services
                    .budget
                    .reserve(capacity * 2 + WORKSPACE_OVERHEAD)
                    .map_err(super::super::system::error)?;
                let output = services
                    .budget
                    .reserve(capacity)
                    .map_err(super::super::system::error)?;
                services.charge()?;
                // 3. 【数据库快照】【变更交付】线程独占预留，成功后绑定新的不可变句柄
                let result = work(&lua, timeout_ms, move |budget| {
                    let _working = working;
                    sqlite::apply(
                        data.as_ref().map(BinaryData::bytes),
                        changes,
                        output,
                        budget,
                    )
                })
                .await?;
                services.check(generation)?;
                Buffer::new(result, services, generation)
            }
        })?,
    )?;
    api.set("sqlite", sqlite)
}

/// 【数据库快照】【原生调度】取消异步等待只撤销计算，实际线程结束后才归还所持额度
/// @param lua 虚拟机；timeout_ms 为单次时限；operation 为持有全部预算租约的工作
/// @returns 当前调用仍有效时交付完整结果
async fn work<T: Send + 'static>(
    lua: &Lua,
    timeout_ms: u64,
    operation: impl FnOnce(sqlite::Budget) -> anyhow::Result<T> + Send + 'static,
) -> mlua::Result<T> {
    // 1. 【数据库快照】【取消绑定】捕获可信执行预算，异步等待释放时发出停止信号
    let execution = super::super::budget::worker(lua)?;
    let cancel = CancellationToken::new();
    let _cancel_on_drop = cancel.clone().drop_guard();
    let budget: sqlite::Budget = Arc::new(move |units| {
        anyhow::ensure!(
            !cancel.is_cancelled(),
            "SQLite snapshot operation cancelled"
        );
        execution(units)
    });
    let running = budget.clone();
    // 2. 【数据库快照】【线程所有权】计算闭包持有全部输入和预留，迟到任务先检查取消
    let worker = tokio::task::spawn_blocking(move || {
        running(0)?;
        let result = operation(running.clone());
        running(0)?;
        result
    });
    // 3. 【数据库快照】【时限交付】排队时间计入局部期限，成功结果仍需复核回调有效期
    let result = tokio::time::timeout(Duration::from_millis(timeout_ms), worker)
        .await
        .map_err(|_| mlua::Error::runtime("SQLite snapshot operation timed out"))?
        .map_err(super::super::system::error)?;
    budget(0).map_err(super::super::system::error)?;
    result.map_err(super::super::system::error)
}

/// 【数据库快照】【句柄归属】拒绝路径、其他实例、过期及已关闭的缓冲
/// @param value Lua 参数；services 为本实例；optional 为是否允许新建空数据库
/// @returns 同一二进制预算下的不可变输入
fn input(
    value: Value,
    services: &Arc<BinaryServices>,
    optional: bool,
) -> mlua::Result<Option<BinaryData>> {
    if optional && value == Value::Nil {
        return Ok(None);
    }
    let Value::UserData(value) = value else {
        return Err(mlua::Error::runtime(
            "SQLite snapshot must be a binary buffer",
        ));
    };
    let buffer = value.borrow::<Buffer>()?;
    let data = buffer.data()?;
    if !data.belongs_to(&services.budget) {
        return Err(mlua::Error::runtime(
            "SQLite snapshot belongs to a different instance",
        ));
    }
    Ok(Some(data))
}
