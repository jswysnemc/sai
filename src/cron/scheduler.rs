use super::repository::CronRepository;
use crate::paths::SaiPaths;
use anyhow::Result;
use chrono::Utc;
use std::time::Duration;

/// 运行 Gateway 专属定时任务调度循环。
pub(crate) async fn run_scheduler(paths: SaiPaths) -> Result<()> {
    let repository = CronRepository::new(&paths)?;
    loop {
        if let Some(job) = repository.next_due(Utc::now().timestamp())? {
            match super::gateway_job::run_gateway_job(&paths, &job).await {
                Ok(_) => repository.complete(&job, Utc::now().timestamp())?,
                Err(error) => repository.fail(&job, &error.to_string(), Utc::now().timestamp())?,
            }
            continue;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
