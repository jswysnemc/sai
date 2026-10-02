use super::{tool_execution::RealToolExecution, Agent, AgentEvent, AgentMode};
use crate::plan::{self, PlanRecord, PlanSubmission};
use crate::question::QuestionResponse;
use anyhow::{bail, Result};

impl Agent {
    /// 【计划模式】【工具执行】参数为工具名、参数与事件回调，返回模型可消费的执行结果。
    pub(super) async fn execute_plan_tool(
        &mut self,
        name: &str,
        arguments: &str,
        on_event: &mut impl FnMut(AgentEvent) -> Result<()>,
    ) -> Result<RealToolExecution> {
        let result = if name == "enter_plan_mode" {
            self.enter_plan().await
        } else {
            self.review_plan(arguments, on_event).await
        };
        Ok(match result {
            Ok(output) => RealToolExecution {
                output,
                model_attachments: vec![],
                failed: false,
            },
            Err(error) => RealToolExecution {
                output: format!("tool error: {error}"),
                model_attachments: vec![],
                failed: true,
            },
        })
    }

    /// 【计划模式】【进入规划】无参数，保存原执行模式并切换只读约束，返回操作结果。
    async fn enter_plan(&self) -> Result<String> {
        if self.mode() != AgentMode::Plan {
            let previous = self.mode();
            let record = PlanRecord {
                title: String::new(),
                plan: String::new(),
                status: "planning".into(),
                execution_mode: previous.key().into(),
                feedback: None,
            };
            plan::store::save(self.state.state_dir(), &record).await?;
            if self.mode() != previous {
                bail!("session mode changed while entering Plan; retry only if still requested");
            }
            self.apply_live_mode(AgentMode::Plan);
        }
        Ok(serde_json::json!({"mode":"plan", "instruction":"Explore read-only, clarify material choices, then call exit_plan_mode with the complete Markdown plan. Do not edit project files or external state."}).to_string())
    }

    /// 【计划模式】【计划审批】参数为完整计划 JSON 与事件回调，返回审批结果；取消或反馈不提升权限。
    async fn review_plan(
        &mut self,
        arguments: &str,
        on_event: &mut impl FnMut(AgentEvent) -> Result<()>,
    ) -> Result<String> {
        if self.mode() != AgentMode::Plan {
            bail!("exit_plan_mode is only available in Plan mode");
        }
        let submission = PlanSubmission::parse(arguments)?;
        let previous = plan::store::load(self.state.state_dir()).await?;
        let target = previous
            .as_ref()
            .and_then(|record| AgentMode::parse(Some(&record.execution_mode)).ok())
            .filter(|mode| *mode != AgentMode::Plan)
            .unwrap_or(AgentMode::Audited);
        let mut record = PlanRecord {
            title: submission.title,
            plan: submission.plan,
            status: "reviewing".into(),
            execution_mode: target.key().into(),
            feedback: None,
        };
        // 1. 【计划模式】【完整审阅】先保存本次正文，再把同一份快照交给所有客户端
        plan::store::save(self.state.state_dir(), &record).await?;
        let (pending, receiver) = crate::question::request_question_with_plan(
            self.session_id(),
            plan::approval_request(target.label()),
            Some(record.plan.clone()),
        );
        let request_id = pending.id.clone();
        if let Err(error) = on_event(AgentEvent::QuestionRequested(pending)) {
            let _ = crate::question::cancel_question(&request_id);
            return Err(error);
        }
        let response = receiver.await.unwrap_or(QuestionResponse::Cancelled);
        on_event(AgentEvent::QuestionResolved {
            request_id,
            response: response.clone(),
        })?;
        let answer = match response {
            QuestionResponse::Answered(answers)
            | QuestionResponse::AnsweredWithImages { answers, .. } => {
                answers.first().and_then(|values| values.first()).cloned()
            }
            _ => None,
        };
        let approved = answer.as_deref() == Some(plan::APPROVE)
            && !self
                .cancel_requested
                .load(std::sync::atomic::Ordering::SeqCst)
            && self.mode() == AgentMode::Plan;
        if !approved {
            record.status = if answer.is_some() {
                "changes_requested"
            } else {
                "cancelled"
            }
            .into();
            record.feedback = answer;
            plan::store::save(self.state.state_dir(), &record).await?;
            return Ok(serde_json::json!({"approved":false,"mode":self.mode().key(),"feedback":record.feedback,"instruction":"Do not implement this plan. Remain in planning and incorporate feedback; do not resubmit unchanged after cancellation."}).to_string());
        }
        // 2. 【计划模式】【批准交接】重建执行工具但保留共享模式句柄，审批通过后才恢复权限
        let registry = self.plan_execution_registry(target)?;
        record.status = "approved".into();
        plan::store::save(self.state.state_dir(), &record).await?;
        if self.mode() != AgentMode::Plan
            || self
                .cancel_requested
                .load(std::sync::atomic::Ordering::SeqCst)
        {
            record.status = "cancelled".into();
            plan::store::save(self.state.state_dir(), &record).await?;
            bail!("session mode changed or the turn stopped before plan execution");
        }
        self.switch_mode(target, registry)?;
        Ok(serde_json::json!({"approved":true,"mode":target.key(),"plan":record.plan,"plan_file":self.state.state_dir().join("plan.json"),"instruction":"The user approved this plan. Continue implementation now, following the approved scope and current execution permissions."}).to_string())
    }

    /// 【计划模式】【执行工具】参数为恢复模式，返回遵守当前档案白名单且共享会话模式的注册表。
    fn plan_execution_registry(&self, mode: AgentMode) -> Result<crate::tools::ToolRegistry> {
        let mut registry =
            crate::tools::builtin_registry_with_cached_mcp(&self.config, &self.paths);
        crate::tools::register_interactive_tools(
            &mut registry,
            &self.config,
            &self.paths,
            self.state.state_dir().display().to_string(),
            self.session_id().to_string(),
        );
        let mut registry = crate::runner::apply_enabled_tools_filter(
            registry,
            &self.config,
            crate::runner::SubmissionSource::Repl,
        )?;
        let audit = Some(crate::permission::PermissionAuditLog::new(
            self.state.state_dir().join("permission-audit.jsonl"),
            self.session_id().to_string(),
        ));
        registry.set_permission_profile(
            crate::permission::PermissionProfile::new(
                mode.permission_profile_mode(),
                crate::runtime_cwd::current_dir()?,
                audit,
            )
            .with_mode_handle(self.live_mode.clone()),
        );
        Ok(registry)
    }
}
