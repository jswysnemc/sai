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

/// 【计划模式】【创建规划】参数为会话目录与普通模式，返回写入结果；清除上一次审批状态。
pub(crate) async fn begin(state_dir: &Path, mode: crate::agent::AgentMode) -> Result<()> {
    save(state_dir, &planning_record(mode)?).await
}

/// 【计划模式】【同步入口】参数为会话目录与普通模式，返回原子写入结果，供终端按键线程使用。
pub(crate) fn begin_sync(state_dir: &Path, mode: crate::agent::AgentMode) -> Result<()> {
    use std::io::Write;
    let record = planning_record(mode)?;
    std::fs::create_dir_all(state_dir)?;
    let mut file = tempfile::NamedTempFile::new_in(state_dir)?;
    file.write_all(&serde_json::to_vec_pretty(&record)?)?;
    file.persist(state_dir.join("plan.json"))?;
    Ok(())
}

/// 【计划模式】【初始状态】参数为执行权限，返回独立规划记录；不允许把 Plan 作为恢复权限。
fn planning_record(mode: crate::agent::AgentMode) -> Result<PlanRecord> {
    anyhow::ensure!(
        mode != crate::agent::AgentMode::Plan,
        "Plan is not an execution permission mode"
    );
    Ok(PlanRecord {
        title: String::new(),
        plan: String::new(),
        status: "planning".into(),
        execution_mode: mode.key().into(),
        feedback: None,
    })
}
