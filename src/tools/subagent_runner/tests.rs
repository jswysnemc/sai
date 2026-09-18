use super::*;
use crate::config::ProviderConfig;
use crate::tools::{ToolProgress, ToolSpec};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::time::Duration;

/// 【子任务】【循环测试】本地模型接口与无副作用工具，记录真实请求及执行次数。
struct Harness {
    runner: SubagentRunner,
    requests: Arc<Mutex<Vec<Value>>>,
    executions: Arc<AtomicUsize>,
    ignore_tool_removal: Arc<AtomicBool>,
    server: tokio::task::JoinHandle<()>,
    _root: tempfile::TempDir,
}

impl Drop for Harness {
    /// 【子任务】【循环测试】测试结束时释放本地接口；无参数、无返回值。
    fn drop(&mut self) {
        self.server.abort();
    }
}

impl Harness {
    /// 【子任务】【循环测试】创建模拟重复调用的执行器。
    /// 参数：`calls` 按轮次生成工具调用，`output` 按执行次数生成结果；返回测试环境。
    async fn new(
        calls: impl Fn(usize) -> Vec<Value> + Send + Sync + 'static,
        output: impl Fn(usize) -> String + Send + Sync + 'static,
    ) -> Self {
        let root = tempfile::tempdir().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = requests.clone();
        let ignore_tool_removal = Arc::new(AtomicBool::new(false));
        let ignore_removal = ignore_tool_removal.clone();
        let calls = Arc::new(calls);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = axum::Router::new().route("/v1/chat/completions", axum::routing::post(
            move |axum::Json(body): axum::Json<Value>| {
                let calls = calls.clone();
                let ignore_removal = ignore_removal.clone();
                let mut requests = recorded.lock().unwrap();
                let round = requests.len();
                requests.push(body.clone());
                async move {
                    let has_tools = body["tools"].as_array().is_some_and(|tools| !tools.is_empty());
                    let calls = if has_tools || ignore_removal.load(Ordering::SeqCst) { calls(round) } else { Vec::new() };
                    let delta = if calls.is_empty() {
                        json!({"content":"审查结束，保留已获得的结果。"})
                    } else {
                        json!({"reasoning_content":"已审查当前包，继续下一项。", "tool_calls":calls})
                    };
                    let response = json!({"choices":[{"index":0,"delta":delta,"finish_reason":"stop"}]});
                    ([("content-type", "text/event-stream")], format!("data: {response}\n\ndata: [DONE]\n\n"))
                }
            },
        ));
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let mut provider = ProviderConfig::new_openai_compatible();
        provider.id = "loop-test".into();
        provider.enabled = true;
        provider.base_url = format!("http://{address}/v1");
        provider.protocol = "openai-chat".into();
        provider.api_key = Some("test-key".into());
        provider.default_model = "test-model".into();
        provider.preserve_thinking = true;
        let config = AppConfig {
            active_provider: provider.id.clone(),
            providers: vec![provider],
            ..Default::default()
        };
        let client =
            OpenAiCompatibleClient::from_config(&config, &SaiPaths::for_tests(root.path()))
                .unwrap();
        let executions = Arc::new(AtomicUsize::new(0));
        let count = executions.clone();
        let output = Arc::new(output);
        let mut tools = ToolRegistry::new();
        tools.register(ToolSpec::new("run_command", "Test command", json!({"type":"object","properties":{"command":{"type":"string"},"justification":{"type":"string"}},"required":["command"]}), move |_| {
            let result = output(count.fetch_add(1, Ordering::SeqCst));
            async move { Ok(result) }
        }));
        let progress = SubagentProgress::new(ToolProgress::default(), ProgressMode::Hidden, false);
        let runner =
            SubagentRunner::new(client, "完成审查后返回结果。", tools, progress).max_steps(12);
        Self {
            runner,
            requests,
            executions,
            ignore_tool_removal,
            server,
            _root: root,
        }
    }

    /// 【子任务】【循环测试】限制测试运行时间；无参数，返回回复与统计。
    async fn run(&self) -> (ChatResult, SubagentStats) {
        tokio::time::timeout(Duration::from_secs(5), self.runner.run("审查软件包"))
            .await
            .expect("子代理应在有限轮次内退出")
            .unwrap()
    }
}

/// 【子任务】【循环测试】生成流式工具调用；参数为序号、名称与参数，返回 JSON。
fn call(round: usize, name: &str, args: Value) -> Value {
    json!({"index":0,"id":format!("call-{round}"),"type":"function","function":{"name":name,"arguments":args.to_string()}})
}

/// 【子任务】【循环测试】生成含变化审计编号的相同命令结果；参数为次数，返回文本。
fn unchanged_output(index: usize) -> String {
    json!({"mode":"foreground","completed":true,"success":true,"exit_code":0,"stdout":"same package data","stderr":"","task_id":format!("audit-{index}")}).to_string()
}

/// 【子任务】【循环测试】重复成功调用必须主动收尾，不能耗尽全部预算；无参数、无返回值。
#[tokio::test]
async fn repeated_calls_finalize_before_exhausting_budget() {
    let harness = Harness::new(
        |round| {
            vec![call(
                round,
                "run_command",
                json!({"command":"cat PKGBUILD"}),
            )]
        },
        unchanged_output,
    )
    .await;
    let (result, stats) = harness.run().await;
    assert!(!stats.budget_reached, "重复拦截后仍在请求模型直到预算耗尽");
    assert!(!result.content.is_empty());
    assert!(harness.requests.lock().unwrap().len() <= 6);
    assert!(harness.executions.load(Ordering::SeqCst) <= 4);
    assert!(stats.loop_detected);
}

/// 【子任务】【循环测试】修改说明或审计编号不能绕过重复防护；无参数、无返回值。
#[tokio::test]
async fn changed_justification_does_not_restart_the_same_command() {
    let harness = Harness::new(
        |round| {
            vec![call(
                round,
                "run_command",
                json!({"command":"cat PKGBUILD", "justification":format!("inspect again {round}")}),
            )]
        },
        unchanged_output,
    )
    .await;
    let (_, stats) = harness.run().await;
    assert!(!stats.budget_reached);
    assert!(
        harness.executions.load(Ordering::SeqCst) <= 4,
        "说明变化导致同一命令重复执行"
    );
}

/// 【子任务】【循环测试】后台任务编号变化不能导致重复启动同一命令；无参数、无返回值。
#[tokio::test]
async fn background_task_ids_do_not_hide_repeated_launches() {
    let harness = Harness::new(|round| vec![call(round, "run_command", json!({"command":"build packages"}))], |index| {
        json!({"mode":"background","ok":true,"task_id":format!("task-{index}"),"task":{"id":index,"status":"running","pid":index}}).to_string()
    }).await;
    let (_, stats) = harness.run().await;
    assert!(stats.loop_detected);
    assert!(!stats.budget_reached);
    assert!(harness.executions.load(Ordering::SeqCst) <= 4);
}

/// 【子任务】【上下文测试】工具轮次必须把推理历史带回下一次真实请求；无参数、无返回值。
#[tokio::test]
async fn tool_round_preserves_reasoning_in_next_request() {
    let harness = Harness::new(
        |round| {
            if round == 0 {
                vec![call(
                    round,
                    "run_command",
                    json!({"command":"cat PKGBUILD"}),
                )]
            } else {
                Vec::new()
            }
        },
        unchanged_output,
    )
    .await;
    harness.run().await;
    let requests = harness.requests.lock().unwrap();
    let assistant = requests[1]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|message| message["role"] == "assistant")
        .unwrap();
    assert_eq!(assistant["reasoning_content"], "已审查当前包，继续下一项。");
}

/// 【子任务】【循环测试】不设置预算时交替重复仍能结束；无参数、无返回值。
#[tokio::test]
async fn alternating_calls_stop_even_without_a_tool_budget() {
    let mut harness = Harness::new(
        |round| {
            vec![call(
                round,
                "run_command",
                json!({"command":format!("cat package-{}/PKGBUILD", round % 2)}),
            )]
        },
        unchanged_output,
    )
    .await;
    harness.runner.max_steps = 0;
    let (_, stats) = harness.run().await;
    assert!(stats.loop_detected);
    assert!(!stats.budget_reached);
    assert_eq!(harness.executions.load(Ordering::SeqCst), 5);
}

/// 【子任务】【循环测试】不同软件包即使输出相同也应继续审查；无参数、无返回值。
#[tokio::test]
async fn different_packages_are_not_treated_as_a_loop() {
    let harness = Harness::new(
        |round| {
            if round < 8 {
                vec![call(
                    round,
                    "run_command",
                    json!({"command":format!("bash -c '\ncat package-{round}/PKGBUILD\n'")}),
                )]
            } else {
                Vec::new()
            }
        },
        unchanged_output,
    )
    .await;
    let (_, stats) = harness.run().await;
    assert!(!stats.loop_detected);
    assert!(!stats.budget_reached);
    assert_eq!(harness.executions.load(Ordering::SeqCst), 8);
}

/// 【子任务】【循环测试】相同调用获得新结果时不能按累计次数拦截；无参数、无返回值。
#[tokio::test]
async fn changing_results_keep_the_task_running() {
    let harness = Harness::new(
        |round| {
            if round < 8 {
                vec![call(
                    round,
                    "run_command",
                    json!({"command":"read progress"}),
                )]
            } else {
                Vec::new()
            }
        },
        |index| format!("completed {index} packages"),
    )
    .await;
    let (_, stats) = harness.run().await;
    assert!(!stats.loop_detected);
    assert_eq!(harness.executions.load(Ordering::SeqCst), 8);
}

/// 【子任务】【循环测试】不存在的工具反复请求时同样收尾；无参数、无返回值。
#[tokio::test]
async fn rejected_tool_calls_stop_without_executing_anything() {
    let harness = Harness::new(
        |round| vec![call(round, "missing_tool", json!({}))],
        unchanged_output,
    )
    .await;
    let (_, stats) = harness.run().await;
    assert!(stats.loop_detected);
    assert!(!stats.budget_reached);
    assert_eq!(harness.executions.load(Ordering::SeqCst), 0);
}

/// 【子任务】【循环测试】畸形调用外壳不能绕过收尾判断；无参数、无返回值。
#[tokio::test]
async fn malformed_invocations_stop_without_executing_anything() {
    let harness = Harness::new(
        |round| vec![call(round, "invoke_tool", json!({"tool_name":null}))],
        unchanged_output,
    )
    .await;
    let (_, stats) = harness.run().await;
    assert!(stats.loop_detected);
    assert!(!stats.budget_reached);
    assert_eq!(harness.executions.load(Ordering::SeqCst), 0);
}

/// 【子任务】【循环测试】模型无视工具撤下时明确退出，不继续循环或宣称完成；无参数、无返回值。
#[tokio::test]
async fn ignored_finalization_returns_an_error_instead_of_looping() {
    let mut harness = Harness::new(
        |round| {
            vec![call(
                round,
                "run_command",
                json!({"command":"cat PKGBUILD"}),
            )]
        },
        unchanged_output,
    )
    .await;
    harness.runner.max_steps = 0;
    harness.ignore_tool_removal.store(true, Ordering::SeqCst);
    let error = tokio::time::timeout(Duration::from_secs(5), harness.runner.run("审查软件包"))
        .await
        .expect("忽略收尾要求仍应结束")
        .err()
        .expect("不能把继续索要工具当成成功");
    assert!(error.to_string().contains("finalization requested tools"));
    assert_eq!(harness.requests.lock().unwrap().len(), 5);
    assert_eq!(harness.executions.load(Ordering::SeqCst), 4);
}

/// 【子任务】【循环测试】批内拦截仍为每条工具调用提供结果；无参数、无返回值。
#[tokio::test]
async fn repeated_batches_preserve_all_tool_result_pairs() {
    let mut harness = Harness::new(
        |round| {
            (0..6)
                .map(|index| {
                    let mut value = call(
                        round * 6 + index,
                        "run_command",
                        json!({"command":"cat PKGBUILD"}),
                    );
                    value["index"] = json!(index);
                    value
                })
                .collect()
        },
        unchanged_output,
    )
    .await;
    harness.runner.max_steps = 0;
    let (_, stats) = harness.run().await;
    assert!(stats.loop_detected);
    assert_eq!(harness.executions.load(Ordering::SeqCst), 4);
    let requests = harness.requests.lock().unwrap();
    let last = requests.last().unwrap();
    let messages = last["messages"].as_array().unwrap();
    for message in messages {
        if let Some(calls) = message["tool_calls"].as_array() {
            for call in calls {
                let matches = messages
                    .iter()
                    .filter(|message| {
                        message["role"] == "tool" && message["tool_call_id"] == call["id"]
                    })
                    .count();
                assert_eq!(matches, 1, "每条调用必须对应一条结果");
            }
        }
    }
}

/// 【子任务】【命令状态测试】非零退出码应累计为失败并显示 failed；无参数、无返回值。
#[tokio::test]
async fn failed_commands_report_failure_in_progress_and_stats() {
    let mut harness = Harness::new(
        |round| {
            if round == 0 {
                vec![call(
                    round,
                    "run_command",
                    json!({"command":"read unavailable package"}),
                )]
            } else {
                Vec::new()
            }
        },
        |_| {
            json!({"mode":"foreground","success":false,"exit_code":1,"stderr":"unavailable"})
                .to_string()
        },
    )
    .await;
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    harness.runner.progress =
        SubagentProgress::new(ToolProgress::new(sender), ProgressMode::Full, true);
    let (_, stats) = harness.run().await;
    assert_eq!(stats.tool_ok, 0);
    assert_eq!(stats.tool_errors, 1);
    let mut found = false;
    while let Ok(event) = receiver.try_recv() {
        if let Some(value) = event.strip_prefix("__subtool_result__") {
            assert_eq!(serde_json::from_str::<Value>(value).unwrap()["ok"], false);
            found = true;
        }
    }
    assert!(found);
}
