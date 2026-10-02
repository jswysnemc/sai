use super::PlanRecord;
use anyhow::Result;
use std::path::Path;

/// 【计划模式】【读取记录】参数为会话目录，返回已有计划；不存在时为空，损坏时返回错误。
pub(crate) async fn load(state_dir: &Path) -> Result<Option<PlanRecord>> {
    match tokio::fs::read(state_dir.join("plan.json")).await {
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// 【计划模式】【保存记录】参数为会话目录与计划记录，返回原子替换结果。
pub(crate) async fn save(state_dir: &Path, record: &PlanRecord) -> Result<()> {
    tokio::fs::create_dir_all(state_dir).await?;
    let temporary = state_dir.join(format!(".plan-{}.tmp", uuid::Uuid::new_v4()));
    tokio::fs::write(&temporary, serde_json::to_vec_pretty(record)?).await?;
    let result = tokio::fs::rename(&temporary, state_dir.join("plan.json")).await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(&temporary).await;
    }
    result?;
    Ok(())
}
