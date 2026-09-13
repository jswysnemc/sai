use anyhow::Result;
use std::sync::Arc;

/// 【插件计算】【预算契约】原生模块共用可跨线程检查的指令、期限及取消额度
pub(crate) type Budget = Arc<dyn Fn(u64) -> Result<()> + Send + Sync>;
