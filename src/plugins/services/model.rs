use super::{PluginServices, CALL_CHAIN};
use crate::config::AppConfig;
use crate::llm::{ChatMessage, ChatStreamEvent, ChatStreamKind, OpenAiCompatibleClient};
use crate::paths::SaiPaths;
use anyhow::{bail, Context, Result};
use sai_plugin_runtime::host::{ModelRequest, ModelResponse, ModelRole, ModelToolCall, ModelUsage};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// 【插件】【模型来源】普通命令延迟解析配置，Agent 调用使用已经选定的客户端。
#[derive(Clone)]
pub(crate) enum PluginModelSource {
    Config(Arc<(AppConfig, SaiPaths)>),
    Client(Arc<OpenAiCompatibleClient>),
}

impl PluginModelSource {
    /// 【插件】【客户端解析】仅在实际模型调用时读取凭据，管理命令无需模型初始化。
    /// @returns 宿主客户端，不向 Lua 传递配置或密钥
    fn resolve(&self) -> Result<OpenAiCompatibleClient> {
        match self {
            Self::Config(config) => OpenAiCompatibleClient::from_config(&config.0, &config.1),
            Self::Client(client) => Ok(client.as_ref().clone()),
        }
    }
}

impl PluginServices {
    /// 【插件】【模型执行】固定本次调用的客户端与工具集合，流式思考继续进入宿主进度通道。
    /// @param request 文本消息与工具名；max_bytes 为输入、输出上限
    /// @returns 单次模型结果和用量，模型建议的工具不会自动执行
    pub(super) async fn complete_model(
        &self,
        request: ModelRequest,
        max_bytes: usize,
    ) -> Result<ModelResponse> {
        let catalog = self.tool_catalog()?;
        for name in &request.tools {
            if !catalog.iter().any(|tool| tool.name == *name) {
                bail!("plugin model tool is not available: {name}");
            }
        }
        let definitions = self
            .tools
            .definitions_for_names(&request.tools.iter().cloned().collect::<BTreeSet<_>>());
        let messages = request
            .messages
            .iter()
            .map(|message| {
                let role = match message.role {
                    ModelRole::System => "system",
                    ModelRole::User => "user",
                    ModelRole::Assistant => "assistant",
                };
                ChatMessage::plain(role, message.content.clone())
                    .with_reasoning(message.reasoning.clone())
            })
            .collect::<Vec<_>>();
        if serde_json::to_vec(&(&messages, &definitions))?.len() > max_bytes {
            bail!("plugin model context exceeds size limit");
        }
        if self.client.get().is_none() {
            let client = self
                .model
                .as_ref()
                .context("plugin model service is unavailable")?
                .resolve()?;
            let _ = self.client.set(client);
        }
        let client = self
            .client
            .get()
            .context("plugin model service is unavailable")?;
        let round = self.rounds.fetch_add(1, Ordering::AcqRel);
        let mut received_bytes = 0usize;
        let mut tool_bytes = BTreeMap::new();
        let operation = client.chat_stream_events(messages, definitions, |event| {
            // 【插件】【流式限额】正文、思考和工具参数共用字节预算，连接未结束也能停止超限响应
            let added = match &event {
                ChatStreamEvent::Chunk(chunk) => chunk.text.len(),
                ChatStreamEvent::ToolCallProgress(progress) => {
                    let bytes = progress
                        .arguments_bytes
                        .saturating_add(progress.name.as_ref().map_or(0, String::len));
                    bytes.saturating_sub(tool_bytes.insert(progress.index, bytes).unwrap_or(0))
                }
            };
            received_bytes = received_bytes.saturating_add(added);
            if received_bytes > max_bytes {
                bail!("plugin model response exceeds size limit");
            }
            if let ChatStreamEvent::Chunk(chunk) = event {
                if request.stream_reasoning && chunk.kind == ChatStreamKind::Reasoning {
                    if let Some(progress) = &self.progress {
                        progress(format!("__subagent_reasoning__{}", chunk.text));
                    }
                }
            }
            Ok(())
        });
        let events = self.events();
        let result = CALL_CHAIN
            .scope(
                self.chain.clone(),
                crate::runtime_cwd::scope(self.workdir.clone(), async {
                    events
                        .model_round(json!({"kind":"plugin", "round":round}), operation)
                        .await
                }),
            )
            .await?;
        let response = ModelResponse {
            content: result.content,
            reasoning: result.reasoning,
            tool_calls: result
                .tool_calls
                .into_iter()
                .map(|call| {
                    // 【插件】【供应商别名】仅还原传输层公开的搜索别名，内部执行别名不能变成普通工具
                    let name = if call.function.name == "sai_web_search"
                        && request.tools.iter().any(|name| name == "web_search")
                    {
                        "web_search".to_string()
                    } else {
                        call.function.name
                    };
                    ModelToolCall {
                        id: call.id,
                        name,
                        arguments: call.function.arguments,
                    }
                })
                .collect(),
            usage: result.usage.map(|usage| ModelUsage {
                prompt_tokens: usage.prompt_tokens,
                completion_tokens: usage.completion_tokens,
                total_tokens: usage.total_tokens,
                cache_read_tokens: usage.cache_read_tokens,
                cache_write_tokens: usage.cache_write_tokens,
            }),
        };
        if serde_json::to_vec(&response)?.len() > max_bytes {
            bail!("plugin model response exceeds size limit");
        }
        Ok(response)
    }
}
