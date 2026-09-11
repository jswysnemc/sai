mod changes;
mod connection;
mod query;
mod validation;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{collections::BTreeMap, sync::Arc};

pub(crate) use changes::apply;
pub(crate) use query::query;
pub(crate) use validation::{validate_changes, validate_query};

pub(crate) const MAX_DATABASE_BYTES: usize = 64 * 1024 * 1024;
pub(crate) const MAX_REQUEST_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_VALUE_BYTES: usize = 256 * 1024;
pub(crate) const WORKSPACE_OVERHEAD: usize = 1024 * 1024;
pub(crate) type Budget = Arc<dyn Fn(u64) -> Result<()> + Send + Sync>;
pub(crate) type Record = BTreeMap<String, Value>;

/// 【数据库快照】【查询契约】只允许单表投影、相等过滤和有界排序分页
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Query {
    pub table: String,
    pub columns: Vec<String>,
    #[serde(default, rename = "where")]
    pub filter: Record,
    #[serde(default)]
    pub order_by: Vec<Order>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

/// 【数据库快照】【分页缺省】每次查询默认最多返回一百条记录
/// @returns 默认行数上限
fn default_limit() -> usize {
    100
}

/// 【数据库快照】【排序契约】列名参与校验，不接受 SQL 表达式
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Order {
    pub column: String,
    #[serde(default)]
    pub descending: bool,
}

/// 【数据库快照】【查询结果】保留声明列顺序，行内值使用完整 JSON 标量
#[derive(Serialize)]
pub(crate) struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Map<String, Value>>,
}

/// 【数据库快照】【字段类型】使用 SQLite 的基础类型亲和性，不增加业务类型
#[derive(Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Kind {
    Integer,
    Real,
    Text,
}

/// 【数据库快照】【列声明】约束来自结构化字段，禁止默认表达式和执行回调
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Column {
    pub name: String,
    pub kind: Kind,
    #[serde(default = "nullable")]
    pub nullable: bool,
}

/// 【数据库快照】【空值缺省】未指定非空约束时允许 SQL null
/// @returns 缺省允许空值
fn nullable() -> bool {
    true
}

/// 【数据库快照】【事务契约】批次只包含有限的声明式表结构与记录变更
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Change {
    CreateTable {
        name: String,
        columns: Vec<Column>,
        primary_key: Option<String>,
        #[serde(default)]
        auto_increment: bool,
    },
    CreateIndex {
        name: String,
        table: String,
        columns: Vec<String>,
        #[serde(default)]
        unique: bool,
    },
    Insert {
        table: String,
        rows: Vec<Record>,
    },
    Upsert {
        table: String,
        key: String,
        rows: Vec<Record>,
    },
    Delete {
        table: String,
        #[serde(default, rename = "where")]
        filter: Record,
    },
}

/// 【数据库快照】【调用限制】调用方只能在包预算内收窄容量和时限
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Options {
    pub max_bytes: Option<usize>,
    pub timeout_ms: Option<u64>,
}
