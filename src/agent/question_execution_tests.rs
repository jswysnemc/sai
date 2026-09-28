use super::*;
use crate::question::{answer_question_with_images, QuestionResponse};

const IMAGE: &str = "data:image/png;base64,aW1hZ2U=";
const ARGUMENTS: &str =
    r#"{"questions":[{"header":"截图","question":"请提供问题截图","options":[]}]}"#;

/// 【结构化提问】【回归环境】构造使用隔离状态的真实 Agent。
/// @param paths 临时路径；返回不访问网络的 Agent
fn test_agent(paths: &SaiPaths) -> Agent {
    let mut config = AppConfig::default();
    config.prompt_sections.state_contract = false;
    config.prompt_sections.mode_reminder = false;
    config.skills.enabled = false;
    config.load_instruction_files = false;
    let state = StateStore::new(paths).unwrap();
    let client = OpenAiCompatibleClient::from_config(&config, paths).unwrap();
    Agent::new(
        config,
        paths,
        state,
        client,
        ToolRegistry::new(),
        AgentMode::Yolo,
    )
    .unwrap()
}

/// 【结构化提问】【图片续接】通过真实待答队列提交图片，验证输出、恢复和压缩后请求。
/// @returns 无；所有状态存放在临时目录
#[tokio::test]
async fn answer_images_survive_question_execution_history_and_compaction() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let agent = test_agent(&paths);
    // 1. 建立可压缩的旧轮次，图片回答属于压缩范围之后的新轮次
    agent.state.start_turn("old", "旧问题").unwrap();
    agent.state.complete_turn("old", "旧回答", None).unwrap();
    let compact = agent.state.select_manual_compaction(0).unwrap().unwrap();
    agent
        .state
        .apply_compaction(&compact, "旧上下文摘要")
        .unwrap();
    agent.state.start_turn("question", "检查截图").unwrap();
    agent
        .state
        .record_tool_call_started("question", 1, "ask", "ask_question", ARGUMENTS)
        .unwrap();
    let mut resolved = None;
    let execution = agent
        .execute_question(ARGUMENTS, &mut |event| {
            match event {
                AgentEvent::QuestionRequested(pending) => {
                    answer_question_with_images(
                        &pending.id,
                        vec![vec!["第一行\n第二行".into()]],
                        vec![vec![IMAGE.into()]],
                    )?;
                }
                AgentEvent::QuestionResolved { response, .. } => resolved = Some(response),
                _ => {}
            }
            Ok(())
        })
        .await
        .unwrap();
    assert!(!execution.failed);
    assert!(matches!(
        resolved,
        Some(QuestionResponse::AnsweredWithImages { .. })
    ));
    assert_eq!(execution.model_attachments.len(), 1);
    assert_eq!(execution.model_attachments[0].image_url, IMAGE);
    assert!(!execution.output.contains(IMAGE));
    let output: serde_json::Value = serde_json::from_str(&execution.output).unwrap();
    assert_eq!(output["answers"][0]["answer"], "第一行\n第二行");
    assert_eq!(output["answers"][0]["image_count"], 1);
    // 2. 使用正式工具结果存储接口保存文字和独立图片
    agent
        .state
        .record_tool_result_completed(
            "question",
            "ask",
            true,
            crate::state::tool_history::ToolResultOutput {
                result_preview: &execution.output,
                result_ref: None,
                error: None,
                original_chars: execution.output.chars().count(),
            },
        )
        .unwrap();
    agent
        .state
        .record_tool_result_images("question", "ask", &execution.model_attachments)
        .unwrap();
    agent
        .state
        .complete_turn("question", "已查看截图", None)
        .unwrap();
    drop(agent);
    // 3. 重建 Agent 后，下一次正式请求仍包含新问题回答和多模态图片
    let mut restored = test_agent(&paths);
    restored.state.start_turn("followup", "继续说明").unwrap();
    let messages = restored
        .chat_messages_for_turn("followup", "继续说明", &[], None, None)
        .unwrap();
    let serialized = serde_json::to_string(&messages).unwrap();
    assert_eq!(serialized.matches(IMAGE).count(), 1);
    assert!(serialized.contains("第一行"));
    assert!(serialized.contains("已查看截图"));
    assert!(serialized.contains("继续说明"));
}

/// 【结构化提问】【无效附件】附件校验失败时仍可重试同一待答请求。
/// @returns 无；不启动模型请求
#[tokio::test]
async fn invalid_images_keep_question_pending_for_retry() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let agent = test_agent(&paths);
    let execution = agent
        .execute_question(ARGUMENTS, &mut |event| {
            if let AgentEvent::QuestionRequested(pending) = event {
                let answers = vec![vec!["截图".into()]];
                assert!(answer_question_with_images(
                    &pending.id,
                    answers.clone(),
                    vec![vec!["invalid".into()]]
                )
                .is_err());
                answer_question_with_images(&pending.id, answers, vec![vec![IMAGE.into()]])?;
            }
            Ok(())
        })
        .await
        .unwrap();
    assert!(!execution.failed);
    assert_eq!(execution.model_attachments.len(), 1);
}
