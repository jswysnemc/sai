use crate::agent::{Agent, AgentMode, ExternalEventBatch, ExternalEventWake};
use crate::config::{AppConfig, ProviderConfig};
use crate::llm::OpenAiCompatibleClient;
use crate::paths::SaiPaths;
use crate::runner::{
    RunnerEvent, RunnerSubmission, SessionRunner, SubmissionSource, UserInputSubmission,
};
use crate::state::StateStore;
use crate::tools::command::{BackgroundCommandStore, BackgroundCommandTask};
use crate::tools::{ToolRegistry, ToolSpec};
use anyhow::Result;
use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// 本地模拟服务下一次请求的响应类型。
pub(super) enum TestResponse {
    Reply,
    Error,
    ToolCall,
}

#[derive(Default)]
struct MockProvider {
    requests: Mutex<Vec<Value>>,
    responses: Mutex<VecDeque<TestResponse>>,
}

/// 隔离的真实会话 Runner、后台任务存储和本地模型服务。
pub(super) struct AutomaticTestHarness {
    pub(super) paths: SaiPaths,
    pub(super) config: AppConfig,
    pub(super) agent: Agent,
    pub(super) events: Vec<RunnerEvent>,
    provider: Arc<MockProvider>,
    server: tokio::task::JoinHandle<()>,
    _temp: tempfile::TempDir,
}

impl AutomaticTestHarness {
    /// 【自动续聊】【测试夹具】创建仅连接本地服务的完整会话。
    ///
    /// 参数: 无
    /// 返回: 使用临时目录、真实后台工具和模拟供应商的测试环境
    pub(super) async fn new() -> Self {
        let provider = Arc::new(MockProvider::default());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = Router::new()
            .route("/v1/chat/completions", post(chat_completion))
            .with_state(provider.clone());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let temp = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(temp.path());
        let mut provider_config = ProviderConfig::default_openai();
        provider_config.base_url = format!("http://{address}/v1");
        provider_config.api_key = Some("test-key".to_string());
        let mut config = AppConfig::default();
        config.active_provider = provider_config.id.clone();
        config.providers = vec![provider_config];
        config.session.auto_title_enabled = false;
        config.skills.enabled = false;
        config.memory.enabled = false;
        config.load_instruction_files = false;
        config.prompt_sections.state_contract = false;
        config.prompt_sections.mode_reminder = false;
        config.prompt_sections.memory_contract = false;
        config.retry.max_attempts = 1;
        let state = StateStore::new(&paths).unwrap();
        state.init_files().unwrap();
        let client = OpenAiCompatibleClient::from_config(&config, &paths).unwrap();
        let mut registry = ToolRegistry::new();
        crate::tools::command::register_session_background(
            &mut registry,
            &config,
            &paths,
            state.session_id(),
        );
        registry.register(ToolSpec::new(
            "probe",
            "Test request boundary",
            json!({"type": "object"}),
            |_| async { Ok("probe completed".to_string()) },
        ));
        let agent = Agent::new(
            config.clone(),
            &paths,
            state,
            client,
            registry,
            AgentMode::Yolo,
        )
        .unwrap();
        Self {
            paths,
            config,
            agent,
            events: Vec::new(),
            provider,
            server,
            _temp: temp,
        }
    }

    /// 【自动续聊】【测试夹具】完成普通对话，使历史以助手回复结尾。
    ///
    /// 参数: 无
    /// 返回: 无；普通请求失败时终止测试
    pub(super) async fn prime(&mut self) {
        self.submit(UserInputSubmission::new("处理后台任务", AgentMode::Yolo))
            .await
            .unwrap();
    }

    /// 【自动续聊】【测试夹具】通过真实 REPL Runner 执行输入并捕获界面事件。
    ///
    /// 参数: input 为本轮输入
    /// 返回: 本轮执行结果；测试超时则失败
    pub(super) async fn submit(&mut self, input: UserInputSubmission) -> Result<()> {
        self.agent.prepare_for_turn()?;
        self.events.clear();
        let runner = SessionRunner::new(&self.paths).with_config(self.config.clone());
        let events = &mut self.events;
        let mut sink = |event| {
            events.push(event);
            Ok(())
        };
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            runner.run_submission_with_agent(
                RunnerSubmission::user_input(SubmissionSource::Repl, input),
                &mut self.agent,
                &mut sink,
            ),
        )
        .await??;
        Ok(())
    }

    /// 【自动续聊】【测试夹具】登记完成任务，并经真实监听器取得待确认通知。
    ///
    /// 参数: 无
    /// 返回: 属于当前会话的后台完成通知
    pub(super) async fn background_notice(&self) -> ExternalEventBatch {
        let store = BackgroundCommandStore::new(self.paths.state_dir.clone());
        store.init().unwrap();
        let stdout = store.logs_dir().join("completed.out");
        let stderr = store.logs_dir().join("completed.err");
        std::fs::write(&stdout, "command output\n").unwrap();
        std::fs::write(&stderr, "").unwrap();
        let task: BackgroundCommandTask = serde_json::from_value(json!({
            "id": "completed-task", "label": "completed command", "command": "test fixture",
            "cwd": ".", "pid": 0, "status": "exited", "stdout_log": stdout,
            "stderr_log": stderr, "started_at": 1, "updated_at": 2, "timeout_seconds": 0,
            "runtime_owner_kind": "session", "runtime_owner_id": self.agent.session_id()
        }))
        .unwrap();
        store.upsert(task).unwrap();
        match self
            .agent
            .external_event_monitor()
            .wait_for_wake()
            .await
            .unwrap()
        {
            Some(ExternalEventWake::Completion(batch)) => batch,
            _ => panic!("完成任务应当产生自动唤醒"),
        }
    }

    /// 【自动续聊】【测试夹具】设置后续请求的响应顺序。
    ///
    /// 参数: responses 为响应类型序列
    /// 返回: 无；序列耗尽后恢复普通回复
    pub(super) fn respond_with(&self, responses: impl IntoIterator<Item = TestResponse>) {
        self.provider.responses.lock().unwrap().extend(responses);
    }

    /// 【自动续聊】【请求验证】统计已发出的请求。
    ///
    /// 参数: 无
    /// 返回: 本地供应商实际收到的请求数
    pub(super) fn request_count(&self) -> usize {
        self.provider.requests.lock().unwrap().len()
    }

    /// 【自动续聊】【请求验证】读取最后一次真实请求。
    ///
    /// 参数: 无
    /// 返回: 请求正文副本
    pub(super) fn last_request(&self) -> Value {
        self.provider
            .requests
            .lock()
            .unwrap()
            .last()
            .unwrap()
            .clone()
    }

    /// 【自动续聊】【请求验证】统计最后一次请求中完整通知出现次数。
    ///
    /// 参数: prompt 为待检查的通知正文
    /// 返回: 用户消息中的匹配数量
    pub(super) fn prompt_occurrences(&self, prompt: &str) -> usize {
        self.last_request()["messages"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|message| message["role"] == "user" && message["content"] == prompt)
            .count()
    }
}

impl Drop for AutomaticTestHarness {
    /// 【自动续聊】【测试清理】停止本地模拟服务。
    ///
    /// 参数: 无
    /// 返回: 无
    fn drop(&mut self) {
        self.server.abort();
    }
}

/// 【自动续聊】【请求验证】忽略空正文后拒绝以助手消息结尾的对话。
///
/// 参数: provider 为测试状态，request 为实际请求正文
/// 返回: 预设流式响应或与故障日志一致的协议错误
async fn chat_completion(
    State(provider): State<Arc<MockProvider>>,
    Json(request): Json<Value>,
) -> Response {
    provider.requests.lock().unwrap().push(request.clone());
    let last_role = request["messages"]
        .as_array()
        .and_then(|messages| {
            messages.iter().rev().find(|message| {
                message["content"]
                    .as_str()
                    .is_some_and(|content| !content.trim().is_empty())
            })
        })
        .and_then(|message| message["role"].as_str());
    if last_role == Some("assistant") {
        return rejection("Requests ending with a model turn are not supported.");
    }
    let response = provider
        .responses
        .lock()
        .unwrap()
        .pop_front()
        .unwrap_or(TestResponse::Reply);
    let body = match response {
        TestResponse::Error => return rejection("Test provider rejected request"),
        TestResponse::Reply => json!({"choices":[{
            "delta":{"content":"后台任务已处理"},"finish_reason":"stop"
        }]}),
        TestResponse::ToolCall => json!({"choices":[{
            "delta":{"tool_calls":[{
                "index":0,"id":"probe-call","type":"function",
                "function":{"name":"probe","arguments":"{}"}
            }]},"finish_reason":"tool_calls"
        }]}),
    };
    (
        [(header::CONTENT_TYPE, "text/event-stream")],
        format!("data: {body}\n\ndata: [DONE]\n\n"),
    )
        .into_response()
}

/// 【自动续聊】【请求验证】构造不可自动重试的模拟供应商错误。
///
/// 参数: message 为错误说明
/// 返回: HTTP 400 响应
fn rejection(message: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({"error": {"code":400,"message":message,"status":"INVALID_ARGUMENT"}})),
    )
        .into_response()
}
