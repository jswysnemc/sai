use super::{client::WeixinBotClient, event::WeixinMessageEvent, server};
use crate::paths::SaiPaths;
use anyhow::Result;
use tokio::{
    sync::{mpsc, Mutex},
    task::JoinSet,
};

/// 【微信网关】【入站调度】参数 paths/client 为运行路径及账号，返回有界队列与自动回收的处理任务。
/// 模型处理保持串行，长轮询独立更新凭证，避免发送等待刷新时阻塞接收
pub(super) fn start(
    paths: &SaiPaths,
    client: &WeixinBotClient,
) -> (mpsc::Sender<WeixinMessageEvent>, JoinSet<Result<()>>) {
    let (sender, mut receiver) = mpsc::channel(128);
    let paths = paths.clone();
    let client = client.clone();
    let workspace = crate::gateways::workspace::gateway_workspace_path(&paths);
    let mut workers = JoinSet::new();
    workers.spawn(crate::runtime_cwd::scope(workspace, async move {
        let http = reqwest::Client::new();
        let lock = Mutex::new(());
        while let Some(event) = receiver.recv().await {
            server::process_message_event(&paths, &client, &http, &lock, event).await?;
        }
        Ok(())
    }));
    (sender, workers)
}
