use super::*;

/// 排队运行更新参数。
#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct QueuedRunUpdate {
    /// 替换后的用户输入。
    #[serde(default)]
    pub(crate) input: Option<String>,
    /// 在当前会话等待队列中的目标位置，从零开始。
    #[serde(default)]
    pub(crate) position: Option<usize>,
    /// 切换排队插入点：请求间隙或轮次结束后。
    #[serde(default)]
    pub(crate) insert_at: Option<QueueInsertAt>,
    /// 替换后的图片附件；缺省表示不改图片。
    #[serde(default)]
    pub(crate) image_urls: Option<Vec<String>>,
}

impl RunManager {
    /// 更新尚未开始的运行内容、附件、插入点或队列位置。
    ///
    /// 参数:
    /// - `run_id`: 排队运行标识
    /// - `update`: 可选输入、图片、位置和插入点
    ///
    /// 返回:
    /// - 更新后的运行摘要
    pub(crate) async fn update_queued(
        &self,
        run_id: &str,
        update: QueuedRunUpdate,
    ) -> Result<ActiveRunInfo> {
        if update.input.is_none()
            && update.position.is_none()
            && update.insert_at.is_none()
            && update.image_urls.is_none()
        {
            bail!("queue update requires input, position, insert_at, or image_urls");
        }

        let _scheduling = self.scheduling.lock().await;
        let mut queues = self.queued.lock().await;
        for queue in queues.values_mut() {
            let Some(current_position) = queue.iter().position(|run| run.info.run_id == run_id)
            else {
                continue;
            };

            // 1. 在副本上完成内容校验和排序，持久化失败时不污染内存队列
            let mut next_queue = queue.clone();
            let mut queued = next_queue
                .remove(current_position)
                .expect("queued run position must exist");
            if let Some(input) = update.input.as_ref() {
                queued.request.input = input.clone();
                queued.info.input = input.clone();
            }
            if let Some(image_urls) = update.image_urls.clone() {
                queued.request.image_url = None;
                queued.request.image_urls = image_urls.clone();
                queued.info.image_urls = image_urls;
            }
            if queued.request.kind == RunKind::Conversation
                && queued.info.input.trim().is_empty()
                && queued.request.image_url.is_none()
                && queued.request.image_urls.is_empty()
            {
                bail!("message cannot be empty");
            }
            if update.input.is_some() || update.image_urls.is_some() {
                validate_start_request(&queued.request)?;
            }
            if let Some(insert_at) = update.insert_at {
                queued.request.insert_at = insert_at;
                queued.info.insert_at = insert_at;
            }
            let target_position = update
                .position
                .unwrap_or(current_position)
                .min(next_queue.len());
            next_queue.insert(target_position, queued.clone());

            // 2. 同步检查点内容和物理顺序，保证重启后仍按新顺序恢复
            let ordered_ids = next_queue
                .iter()
                .map(|run| run.info.run_id.clone())
                .collect::<Vec<_>>();
            self.checkpoints.update_queued(
                run_id,
                &queued.info.input,
                queued.request.insert_at,
                &queued.info.image_urls,
                &ordered_ids,
            )?;
            *queue = next_queue;
            let info = queued.info.clone();
            drop(queues);

            // 3. 通知已订阅客户端更新内容和本地顺序
            let bus = self.session_bus(&info.workspace_id, &info.session_id).await;
            let _ = bus.emit(WebEvent::new(
                &info.run_id,
                &info.workspace_id,
                &info.session_id,
                "run.queue.updated",
                json!({
                    "input": info.input,
                    "position": target_position,
                    "insert_at": info.insert_at,
                    "image_urls": info.image_urls,
                }),
            ));
            return Ok(info);
        }
        bail!("queued run not found: {run_id}")
    }
}
