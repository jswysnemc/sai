use super::external_events::ExternalEventPoll;
use super::{
    Agent, AgentEvent, ExternalEventBatch, ExternalEventWake, InterMessage, InterMessageEvent,
    InterMessageKind, InterMessageSource,
};
use crate::llm::{ChatMessage, ChatResult};
use crate::state::turn_messages::{NewTurnMessage, TurnMessageKind};
use anyhow::Result;
use std::time::Duration;

const MESSAGE_GAP_POLL_INTERVAL: Duration = Duration::from_millis(250);

/// 一条间隙消息成功进入模型请求后需要确认的来源。
pub(super) enum GapMessageAck {
    None,
    External(ExternalEventBatch),
    Source(String),
}

/// 已写入当前轮次但尚未由 provider 成功消费的消息。
pub(super) struct PendingGapDelivery {
    pub(super) persisted_id: String,
    pub(super) ack: GapMessageAck,
}

/// 已选出的间隙消息及其确认来源。
pub(super) struct GapMessageCandidate {
    message: InterMessage,
    ack: GapMessageAck,
}

impl Agent {
    /// 选择下一条模型间隙消息。
    ///
    /// 1. 用户排队的请求间隔消息优先于自动完成回执
    /// 2. 每次只返回一条消息
    /// 3. 外部完成（含 mesh 入站）走请求间隔，在下一次模型请求前注入
    /// 4. 只有最终回复边界允许等待后台任务或生成 Goal 续作
    ///
    /// 参数:
    /// - `source`: 可选用户排队消息来源
    /// - `wait_for_external`: 是否等待仍在运行的后台工作
    /// - `allow_goal_continuation`: 是否允许生成 Goal 自动续作消息
    /// - `on_event`: Agent 事件接收器
    ///
    /// 返回:
    /// - 下一条消息；当前没有可投递内容时返回空
    pub(super) async fn next_gap_message<F>(
        &self,
        source: Option<&dyn InterMessageSource>,
        wait_for_external: bool,
        allow_goal_continuation: bool,
        on_event: &mut F,
    ) -> Result<Option<GapMessageCandidate>>
    where
        F: FnMut(AgentEvent) -> Result<()>,
    {
        let monitor = self.external_event_monitor();
        let can_poll_external = self.tools.contains("subagent")
            || self.tools.contains("background_command")
            || self.tools.contains("mesh_send");
        let mut wait_announced = false;
        loop {
            // 1. 用户新要求具有最高优先级，并且保持 Web 队列中的可编辑顺序
            if let Some(source) = source {
                if let Some(message) = source.peek().await? {
                    let id = message.id.clone();
                    return Ok(Some(GapMessageCandidate {
                        message,
                        ack: GapMessageAck::Source(id),
                    }));
                }
            }
            if !can_poll_external {
                return Ok(None);
            }

            // 2. 外部监听器已经把完成事件压缩为单条确定性回执。
            //    mesh 入站也走这条请求间隙，不排到轮次结束后才投递。
            match monitor.poll_once().await? {
                ExternalEventPoll::Ready(ExternalEventWake::Completion(batch)) => {
                    let prompt = self.external_completion_prompt(&batch)?;
                    let message = InterMessage {
                        id: batch.event_id().to_string(),
                        kind: InterMessageKind::ExternalCompletion,
                        prompt,
                        display: batch.display().to_string(),
                        image_urls: Vec::new(),
                    };
                    return Ok(Some(GapMessageCandidate {
                        message,
                        ack: GapMessageAck::External(batch),
                    }));
                }
                ExternalEventPoll::Ready(ExternalEventWake::GoalContinuation)
                    if allow_goal_continuation =>
                {
                    let Some(goal) = self.state.goal()?.filter(|goal| goal.status.is_active())
                    else {
                        return Ok(None);
                    };
                    return Ok(Some(GapMessageCandidate {
                        message: InterMessage {
                            id: format!("goal_{}_{}", goal.id, uuid::Uuid::new_v4().simple()),
                            kind: InterMessageKind::GoalContinuation,
                            prompt: crate::goal::continuation_prompt(&goal),
                            display: format!("Goal 自动续轮：{}", goal.objective),
                            image_urls: Vec::new(),
                        },
                        ack: GapMessageAck::None,
                    }));
                }
                ExternalEventPoll::Ready(ExternalEventWake::GoalContinuation) => return Ok(None),
                ExternalEventPoll::Waiting if wait_for_external => {
                    if !wait_announced {
                        on_event(AgentEvent::WaitingExternal)?;
                        wait_announced = true;
                    }
                    tokio::time::sleep(MESSAGE_GAP_POLL_INTERVAL).await;
                }
                ExternalEventPoll::Waiting | ExternalEventPoll::Idle => return Ok(None),
            }
        }
    }

    /// 把选中的消息写入当前轮次和内存上下文。
    ///
    /// 参数:
    /// - `turn_id`: 当前持久化轮次标识
    /// - `after_tool_seq`: 消息前已经完成的工具数量
    /// - `candidate`: 待插入消息
    /// - `messages`: 当前 provider 上下文
    /// - `on_event`: Agent 事件接收器
    ///
    /// 返回:
    /// - 等待下一次成功请求确认的交付记录
    pub(super) fn inject_gap_message<F>(
        &self,
        turn_id: &str,
        after_tool_seq: usize,
        candidate: GapMessageCandidate,
        messages: &mut Vec<ChatMessage>,
        on_event: &mut F,
    ) -> Result<PendingGapDelivery>
    where
        F: FnMut(AgentEvent) -> Result<()>,
    {
        let kind = match candidate.message.kind {
            InterMessageKind::GoalContinuation => TurnMessageKind::GoalContinuation,
            InterMessageKind::ExternalCompletion => TurnMessageKind::ExternalCompletion,
            InterMessageKind::QueuedUser => TurnMessageKind::QueuedUser,
        };
        let persisted = self.state.record_turn_message(NewTurnMessage {
            turn_id: turn_id.to_string(),
            after_tool_seq,
            kind,
            model_content: candidate.message.prompt.clone(),
            display_content: candidate.message.display.clone(),
            reasoning: None,
            image_urls: candidate.message.image_urls.clone(),
        })?;
        let provider_message = if candidate.message.image_urls.is_empty() {
            ChatMessage::plain("user", candidate.message.prompt.clone())
        } else {
            ChatMessage::user_with_images(
                candidate.message.prompt.clone(),
                candidate.message.image_urls.clone(),
            )
        };
        messages.push(provider_message);
        on_event(AgentEvent::InterMessage(InterMessageEvent {
            id: candidate.message.id,
            kind: candidate.message.kind,
            content: candidate.message.display,
        }))?;
        Ok(PendingGapDelivery {
            persisted_id: persisted.id,
            ack: candidate.ack,
        })
    }

    /// 保存需要继续对话的非工具助手消息。
    ///
    /// 参数:
    /// - `turn_id`: 当前持久化轮次标识
    /// - `after_tool_seq`: 消息前已经完成的工具数量
    /// - `result`: provider 返回的助手消息
    /// - `messages`: 当前 provider 上下文
    ///
    /// 返回:
    /// - 写入是否成功
    pub(super) fn persist_intermediate_assistant(
        &self,
        turn_id: &str,
        after_tool_seq: usize,
        result: &ChatResult,
        messages: &mut Vec<ChatMessage>,
    ) -> Result<()> {
        self.state.record_turn_message(NewTurnMessage {
            turn_id: turn_id.to_string(),
            after_tool_seq,
            kind: TurnMessageKind::Assistant,
            model_content: result.content.clone(),
            display_content: result.content.clone(),
            reasoning: result.reasoning.clone(),
            image_urls: Vec::new(),
        })?;
        messages.push(
            ChatMessage::assistant(result.content.clone(), None)
                .with_reasoning(result.reasoning.clone()),
        );
        Ok(())
    }

    /// 确认间隙消息已经进入成功的 provider 请求。
    ///
    /// 参数:
    /// - `delivery`: 待确认交付
    /// - `source`: 可选用户排队消息来源
    ///
    /// 返回:
    /// - 确认是否成功
    pub(super) async fn acknowledge_gap_delivery(
        &self,
        delivery: PendingGapDelivery,
        source: Option<&dyn InterMessageSource>,
    ) -> Result<()> {
        match delivery.ack {
            GapMessageAck::None => Ok(()),
            GapMessageAck::External(batch) => self.acknowledge_external_events(&batch),
            GapMessageAck::Source(message_id) => match source {
                Some(source) => source.acknowledge(&message_id).await,
                None => Ok(()),
            },
        }
    }

    /// 撤销未进入成功 provider 请求的间隙消息。
    ///
    /// 参数:
    /// - `delivery`: 未确认交付
    ///
    /// 返回:
    /// - 删除是否成功
    pub(super) fn rollback_gap_delivery(&self, delivery: &PendingGapDelivery) -> Result<()> {
        self.state.remove_turn_message(&delivery.persisted_id)?;
        Ok(())
    }

    /// 为 Goal 场景补齐继续目标提示。
    ///
    /// 参数:
    /// - `batch`: 单条外部完成事件
    ///
    /// 返回:
    /// - 发送给模型的用户消息
    fn external_completion_prompt(&self, batch: &ExternalEventBatch) -> Result<String> {
        let mut prompt = self
            .state
            .goal()?
            .filter(|goal| goal.status.accepts_external_wake())
            .map(|goal| crate::goal::continuation_prompt(&goal))
            .unwrap_or_default();
        if !prompt.is_empty() {
            prompt.push_str("\n\n");
        }
        prompt.push_str(batch.prompt());
        Ok(prompt)
    }
}
