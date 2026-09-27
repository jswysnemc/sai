use super::*;
use crate::jev::{self, Candidate, JevClient, Selection, SelectionLimits};
use crate::llm::ToolCall;

impl Agent {
    /// 【Jev路由】【请求前预选】在用户消息发往模型前，由 Jev 判断需要暴露的工具与 skill。
    ///
    /// 结果作为本轮注入文本并入用户消息，随轮次持久化并在后续回放；
    /// 不伪造 assistant 工具调用，避免思考型模型因缺少 reasoning 拒绝请求。
    /// Jev 失败或未选中任何资源时返回空，本轮只暴露基础工具。
    ///
    /// 参数:
    /// - `turn_id`: 当前轮次标识，用于排除运行中轮次读取近期历史
    /// - `input`: 用户本轮输入
    ///
    /// 返回:
    /// - 需要注入用户消息的暴露结果块
    pub(super) async fn jev_preselect(&mut self, turn_id: &str, input: &str) -> Option<String> {
        if !self.tools_enabled || !self.tool_visibility.is_jev_routing() || input.trim().is_empty()
        {
            return None;
        }
        // 1. 读取近期历史作为判断背景
        let history = match self.chat_base_context_projection(Some(turn_id)) {
            Ok(projection) => projection.messages,
            Err(error) => {
                eprintln!("【Jev路由】【请求前预选】读取历史失败: {error:#}");
                Vec::new()
            }
        };
        // 2. 请求 Jev 判断，失败时退回基础工具，不阻断对话
        let selection = match self.jev_decide(input, &history).await {
            Ok(selection) if !selection.is_empty() => selection,
            Ok(_) => return None,
            Err(error) => {
                eprintln!("【Jev路由】【请求前预选】判断失败，本轮仅暴露基础工具: {error:#}");
                return None;
            }
        };
        // 3. 标记暴露并包装为注入块
        match self.tool_visibility.expose_selection(
            &self.tools,
            &selection,
            &self.config,
            &self.paths,
        ) {
            Ok(output) => Some(preselect_block(&output)),
            Err(error) => {
                eprintln!("【Jev路由】【请求前预选】暴露资源失败: {error:#}");
                None
            }
        }
    }

    /// 【Jev路由】【能力申请】执行模型发起的 `request_capability` 调用并回写结果。
    ///
    /// 参数:
    /// - `turn_id`: 当前轮次标识
    /// - `recorded_call`: 供应商原始工具调用，用于历史记录
    /// - `call`: 解包后的工具调用
    /// - `messages`: 当前请求上下文，结果追加到末尾
    /// - `repeat_guard`: 重复调用统计，失败调用计入拒绝次数
    /// - `on_event`: 流式事件回调
    ///
    /// 返回:
    /// - 历史写入或事件回调失败时返回错误；Jev 失败以工具错误形式交给模型
    pub(super) async fn respond_capability_request<F>(
        &mut self,
        turn_id: &str,
        recorded_call: &ToolCall,
        call: ToolCall,
        messages: &mut Vec<ChatMessage>,
        repeat_guard: &mut repeat_guard::RepeatGuard,
        on_event: &mut F,
    ) -> Result<()>
    where
        F: FnMut(AgentEvent) -> Result<()>,
    {
        // 1. 请求 Jev 并转换失败为工具错误
        let (ok, output) = match self
            .handle_capability_request(&call.function.arguments, messages)
            .await
        {
            Ok(output) => (true, output),
            Err(error) => (false, tool_error_output(&error)),
        };
        if !ok {
            repeat_guard.observe_rejected(&call.function.name, &call.function.arguments);
        }
        // 2. 推送事件、持久化并追加到上下文
        on_event(AgentEvent::ToolResult {
            name: call.function.name.clone(),
            ok,
            output: output.clone(),
        })?;
        let context_output = tools::tool_output_for_context(&call.function.name, &output);
        self.record_tool_result_completed(turn_id, recorded_call, ok, &output, &context_output)?;
        messages.push(ChatMessage::tool(call.id, context_output));
        Ok(())
    }

    /// 【Jev路由】【能力申请】处理模型发起的 `request_capability` 调用。
    ///
    /// 参数:
    /// - `arguments`: 工具参数 JSON
    /// - `messages`: 当前请求上下文，用作 Jev 的判断背景
    ///
    /// 返回:
    /// - 给模型的暴露结果；参数错误、未开启或 Jev 失败时返回错误
    pub(super) async fn handle_capability_request(
        &mut self,
        arguments: &str,
        messages: &[ChatMessage],
    ) -> Result<String> {
        if !self.tool_visibility.is_jev_routing() {
            anyhow::bail!("request_capability is only available when jev.routing is enabled");
        }
        // 1. 解析需求并请求 Jev 判断
        let request = tools::jev_request::CapabilityRequest::parse(arguments)?;
        let selection = self.jev_decide(&request.need, messages).await?;
        // 2. 标记暴露；未选中时同样返回说明，便于模型调整描述
        self.tool_visibility
            .expose_selection(&self.tools, &selection, &self.config, &self.paths)
    }

    /// 【Jev路由】【判断】对全部未暴露候选各问一个 Noul，并按阈值与上限筛选。
    ///
    /// 参数:
    /// - `need`: 用户请求或模型描述的能力需求
    /// - `messages`: 当前请求上下文
    ///
    /// 返回:
    /// - Jev 选中的资源；没有候选时返回空选择
    async fn jev_decide(&self, need: &str, messages: &[ChatMessage]) -> Result<Selection> {
        let settings = &self.config.jev.routing;
        // 1. 收集候选，已暴露的资源不再参与判断
        let candidates: Vec<Candidate> =
            self.tool_visibility
                .jev_candidates(&self.tools, &self.config, &self.paths);
        if candidates.is_empty() {
            return Ok(Selection::default());
        }
        // 2. 组装共享背景：近期对话不含系统提示
        let dialogue = messages
            .iter()
            .filter(|message| message.role != "system")
            .cloned()
            .collect::<Vec<_>>();
        let conversation =
            crate::permission::build_audit_context(&dialogue, settings.context_chars);
        let available = self.tool_visibility.exposed_resource_names(&self.tools);
        let state = jev::build_state(need, &conversation, &available);
        // 3. 一次请求批量评估全部候选
        let connection = self.config.jev_connection()?;
        let client = JevClient::new(&connection, settings.timeout_seconds)?;
        let answers = client
            .evaluate_nouls(&state, &jev::build_questions(&candidates))
            .await?;
        debug_scores(&candidates, &answers);
        Ok(jev::select(
            &candidates,
            &answers,
            SelectionLimits {
                threshold: settings.threshold,
                max_tools: settings.max_tools,
                max_skills: settings.max_skills,
            },
        ))
    }
}

/// 注册渐进网关；Jev 暴露决策开启时改用不列目录的 `load` 并注册 `request_capability`。
///
/// 参数:
/// - `tools`: 会话工具注册表
/// - `visibility`: 当前可见性状态
///
/// 返回:
/// - 无
pub(super) fn register_tool_gateways(tools: &mut ToolRegistry, visibility: &ToolVisibility) {
    let deferred = visibility.deferred_tools().to_vec();
    if visibility.is_jev_routing() {
        tools::progressive::register_jev_loader(tools, &deferred);
        tools::jev_request::register(tools);
    } else {
        tools::register_progressive_loader(tools, &deferred);
    }
}

/// 设置 `SAI_JEV_DEBUG` 时输出概率最高的候选，便于调节阈值。
///
/// 参数:
/// - `candidates`: 本次判断的候选
/// - `answers`: 问题 id 到"是"概率的映射
///
/// 返回:
/// - 无
fn debug_scores(candidates: &[Candidate], answers: &std::collections::BTreeMap<String, f64>) {
    if std::env::var_os("SAI_JEV_DEBUG").is_none() {
        return;
    }
    let mut scored = candidates
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            answers
                .get(&jev::question_id(index))
                .map(|probability| (*probability, candidate.name.as_str()))
        })
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| right.0.total_cmp(&left.0));
    scored.truncate(8);
    eprintln!(
        "【Jev路由】【候选概率】候选数 {}，最高: {:?}",
        candidates.len(),
        scored
    );
}

/// 把预选结果包装为用户消息中的注入块。
///
/// 参数:
/// - `output`: `expose_selection` 生成的 JSON 结果
///
/// 返回:
/// - 带标签的注入文本
fn preselect_block(output: &str) -> String {
    format!(
        "<jev-exposed-capabilities>\nBefore this request, the Jev router exposed the tools and skills below for the user's message. They stay exposed for the rest of the conversation; call request_capability for anything else.\n{}\n</jev-exposed-capabilities>",
        output.trim()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preselect_block_wraps_output_in_tag() {
        let block = preselect_block("{\"ok\":true}\n");
        assert!(block.starts_with("<jev-exposed-capabilities>"));
        assert!(block.contains("{\"ok\":true}\n</jev-exposed-capabilities>"));
    }
}
