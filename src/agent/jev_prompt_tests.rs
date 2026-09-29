use super::*;
use crate::jev::{CandidateKind, Selection};

/// 创建隔离的 Agent；参数为配置与路径，返回 Agent。
fn agent(config: AppConfig, paths: &SaiPaths) -> Agent {
    let state = StateStore::new(paths).unwrap();
    let client = OpenAiCompatibleClient::from_config(&config, paths).unwrap();
    Agent::new_with_extra_system_prompt(
        config,
        paths,
        state,
        client,
        ToolRegistry::new(),
        AgentMode::Yolo,
        Some("附加静态<jev description=\"附加场景\">附加隐藏</jev>"),
    )
    .unwrap()
}

/// 验证静态提示和指令更新不泄露片段，刷新后仍能路由附加提示，无参数、无返回值。
#[test]
fn baseline_and_live_instruction_updates_exclude_deferred_content() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    std::fs::create_dir_all(&paths.config_dir).unwrap();
    std::fs::write(
        paths.config_dir.join("AGENT.md"),
        "文件静态<jev>文件隐藏</jev>",
    )
    .unwrap();
    let mut config = AppConfig::default();
    config.jev.routing.enabled = true;
    config.system_prompt = Some("配置静态<jev description=\"配置场景\">配置隐藏</jev>".into());
    config.plugins.memory.jev_injection = true;
    let mut agent = agent(config, &paths);
    for hidden in [
        "文件隐藏",
        "配置隐藏",
        "附加隐藏",
        crate::memory::file_store::memory_contract(),
    ] {
        assert!(!agent.base_system_prompt.contains(hidden));
    }
    assert!(agent.base_system_prompt.contains("配置静态"));
    assert!(agent.base_system_prompt.contains("文件静态"));
    agent.prepare_for_turn().unwrap();
    assert!(agent.base_system_prompt.contains("附加静态"));
    let context = agent.jev_prompt_context(Some("记忆索引内容")).unwrap();
    assert!(context
        .candidates
        .iter()
        .all(|item| item.kind == CandidateKind::Prompt));
    assert_eq!(context.candidates.len(), 4);
    let selected = Selection {
        prompts: context.candidates.iter().map(|c| c.name.clone()).collect(),
        ..Default::default()
    };
    let rendered = context.render(&selected);
    assert!(rendered.memory_selected);
    for hidden in ["文件隐藏", "配置隐藏", "附加隐藏"] {
        assert!(rendered.block.as_deref().unwrap().contains(hidden));
    }
    assert!(!rendered.block.unwrap().contains("记忆索引内容"));
    assert!(context.render(&Selection::default()).block.is_none());
    std::fs::write(
        paths.config_dir.join("AGENT.md"),
        "文件已更新<jev>新增隐藏</jev>",
    )
    .unwrap();
    let projection = agent.chat_base_context_projection(None).unwrap();
    let serialized = serde_json::to_string(&projection.messages).unwrap();
    assert!(!serialized.contains("新增隐藏"));
    assert!(agent
        .jev_prompt_context(None)
        .unwrap()
        .candidates
        .iter()
        .any(|c| c.description == "新增隐藏"));
}

/// 验证旧配置默认静态注入，路由关闭时新开关不改变行为，无参数、无返回值。
#[test]
fn memory_configuration_is_backward_compatible() {
    let legacy: crate::config::MemoryConfig = serde_json::from_str("{}").unwrap();
    assert!(!legacy.jev_injection);
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let mut config = AppConfig::default();
    config.plugins.memory.jev_injection = true;
    config.system_prompt = Some("<jev>静态回退</jev>".into());
    assert!(!config.jev_memory_injection_active());
    let prompt = build_base_system_prompt(&config, &paths, true, None).unwrap();
    assert!(prompt.contains("静态回退"));
    assert!(prompt.contains(crate::memory::file_store::memory_contract()));
    config.jev.routing.enabled = true;
    let prompt = build_base_system_prompt(&config, &paths, true, None).unwrap();
    assert!(!prompt.contains(crate::memory::file_store::memory_contract()));
    config.plugins.memory.jev_injection = false;
    let prompt = build_base_system_prompt(&config, &paths, true, None).unwrap();
    assert!(prompt.contains(crate::memory::file_store::memory_contract()));
}

/// 构造本地 Jev HTTP 响应；参数为请求 JSON，返回可控的概率或服务错误。
pub(super) async fn route_response(
    axum::Json(request): axum::Json<serde_json::Value>,
) -> (axum::http::StatusCode, axum::Json<serde_json::Value>) {
    use serde_json::json;
    let need = request["state"]["request"].as_str().unwrap();
    if need == "fail" {
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            axum::Json(json!({"error": "unavailable"})),
        );
    }
    let answers = request["questions"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(id, question)| {
            let capability = &question["instructions"]["capability"];
            let description = capability["description"].as_str().unwrap();
            let selected = need == "all" || (need == "prompt" && description == "配置场景");
            (
                id.clone(),
                json!({"noul": if selected { 0.99 } else { 0.01 }}),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    (
        axum::http::StatusCode::OK,
        axum::Json(json!({"answers": answers})),
    )
}

/// 验证每轮 HTTP 判断、未命中、错误回退及无工具模式，无参数、无返回值。
#[tokio::test]
async fn http_routing_controls_prompt_and_memory_exposure_each_turn() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            axum::Router::new().route("/jev", axum::routing::post(route_response)),
        )
        .await
        .unwrap();
    });
    let temp = tempfile::tempdir().unwrap();
    crate::runtime_cwd::scope(temp.path().to_path_buf(), async {
        let paths = SaiPaths::for_tests(temp.path());
        let mut config = AppConfig::default();
        config.jev.routing.enabled = true;
        config.plugins.memory.jev_injection = true;
        config.tools.enabled = false;
        config.load_instruction_files = false;
        config.system_prompt = Some("baseline<jev description=\"配置场景\">条件正文</jev>".into());
        config.model_endpoints.push(serde_json::from_value(serde_json::json!({
            "id": "local-jev", "kind": "jev", "name": "Local", "endpoint": format!("http://{address}/jev"), "api_key": "test"
        })).unwrap());
        let mut agent = agent(config, &paths);
        for (index, (need, prompt_expected, memory_expected)) in [("none", false, false), ("prompt", true, false), ("all", true, true), ("fail", false, false), ("none", false, false), ("all", true, true)].into_iter().enumerate() {
            let turn = format!("route-{index}");
            agent.state.start_turn(&turn, need).unwrap();
            let mut phases = Vec::new();
            let result = agent.jev_preselect(&turn, need, Some("<memory-index>索引</memory-index>"), &mut |event| {
                if let AgentEvent::JevPreselect { phase, .. } = event { phases.push(phase); }
                Ok(())
            }).await.unwrap();
            assert_eq!(result.memory_selected, memory_expected, "{need}");
            assert_eq!(result.block.as_deref().is_some_and(|block| block.contains("条件正文")), prompt_expected, "{need}");
            let memory = result.memory_selected.then_some("<memory-index>索引</memory-index>");
            let messages = agent.chat_messages_for_turn(&turn, need, &[], memory, result.block.as_deref()).unwrap();
            let current = serde_json::to_string(messages.last().unwrap()).unwrap();
            assert_eq!(current.contains("条件正文"), prompt_expected);
            assert!(!serde_json::to_string(&messages[0]).unwrap().contains("条件正文"));
            if need == "fail" { assert_eq!(phases, ["running", "failed"]); }
            agent.state.complete_turn(&turn, "answer", None).unwrap();
        }
    }).await;
    server.abort();
}

/// 验证禁用记忆与契约、白名单阻断时不产生记忆契约候选，无参数、无返回值。
#[test]
fn disabled_memory_and_contract_do_not_create_candidates() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let mut config = AppConfig::default();
    config.jev.routing.enabled = true;
    config.plugins.memory.jev_injection = true;
    config.plugins.memory.enabled = false;
    let mut agent = agent(config, &paths);
    assert!(!agent
        .jev_prompt_context(None)
        .unwrap()
        .candidates
        .iter()
        .any(|c| c.name == "memory_context"));
    agent.config.plugins.memory.enabled = true;
    agent.config.prompt_sections.memory_contract = false;
    assert!(!agent
        .jev_prompt_context(Some("index"))
        .unwrap()
        .candidates
        .iter()
        .any(|c| c.name == "memory_context"));
    agent.config.prompt_sections.memory_contract = true;
    agent.config.agent_runtime = Some(crate::config::AgentRuntimeOverride {
        enabled_tools: vec!["read_file".into()],
        ..Default::default()
    });
    assert!(!agent
        .jev_prompt_context(None)
        .unwrap()
        .candidates
        .iter()
        .any(|c| c.name == "memory_context"));
}

/// 验证已有会话启用路由后，冻结的指令 baseline 不再泄露标签内容，无参数、无返回值。
#[test]
fn enabling_routing_filters_a_preexisting_instruction_baseline() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    std::fs::create_dir_all(&paths.config_dir).unwrap();
    std::fs::write(
        paths.config_dir.join("AGENT.md"),
        "规则<jev>旧基线隐藏内容</jev>",
    )
    .unwrap();
    let mut config = AppConfig {
        system_prompt: Some("静态配置".into()),
        ..Default::default()
    };
    let state = StateStore::new(&paths).unwrap();
    let old = build_base_system_prompt(&config, &paths, true, None).unwrap();
    state.reset_if_prompt_changed(&old).unwrap();
    assert!(state
        .context_epoch_baseline()
        .unwrap()
        .unwrap()
        .contains("旧基线隐藏内容"));
    config.jev.routing.enabled = true;
    let client = OpenAiCompatibleClient::from_config(&config, &paths).unwrap();
    let agent = Agent::new(
        config,
        &paths,
        state,
        client,
        ToolRegistry::new(),
        AgentMode::Yolo,
    )
    .unwrap();
    let messages = agent.chat_base_context_projection(None).unwrap().messages;
    assert!(!serde_json::to_string(&messages)
        .unwrap()
        .contains("旧基线隐藏内容"));
}
