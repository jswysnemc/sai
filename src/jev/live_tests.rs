use super::*;
use crate::config::{AppConfig, JevAuditConfig, JevRoutingConfig};

/// 构造接近真实会话的候选集合。
fn roster() -> Vec<Candidate> {
    vec![
        Candidate::new(
            CandidateKind::Tool,
            "web_search",
            "Search the web for current information.",
        ),
        Candidate::new(
            CandidateKind::Tool,
            "web_fetch",
            "Fetch a URL and return its readable content.",
        ),
        Candidate::new(
            CandidateKind::Tool,
            "analyze_image",
            "Analyze an image file with a vision model.",
        ),
        Candidate::new(
            CandidateKind::Tool,
            "cron",
            "Create, list and delete scheduled tasks.",
        ),
        Candidate::new(
            CandidateKind::Skill,
            "drawio",
            "Create flowcharts, architecture and sequence diagrams as .drawio files.",
        ),
        Candidate::new(
            CandidateKind::Skill,
            "qq-mail",
            "Send email through QQ Mail SMTP with attachments.",
        ),
    ]
}

/// 用真实 Jev 请求检查问题设计：返回每个候选的概率与最终选择。
async fn decide(need: &str) -> (Vec<(String, f64)>, Selection) {
    let config = JevRoutingConfig::default();
    let connection = AppConfig::default().jev_connection().unwrap();
    let client = JevClient::new(&connection, config.timeout_seconds).unwrap();
    let candidates = roster();
    let state = build_state(need, "", &["read_file".into(), "run_command".into()]);
    let answers = client
        .evaluate_nouls(&state, &build_questions(&candidates))
        .await
        .unwrap();
    let scored = candidates
        .iter()
        .enumerate()
        .map(|(index, item)| (item.name.clone(), answers[&question_id(index)]))
        .collect();
    let limits = SelectionLimits {
        threshold: config.threshold,
        max_tools: 6,
        max_skills: 3,
    };
    (scored, select(&candidates, &answers, limits))
}

#[tokio::test]
#[ignore = "requires TYPESAFE_API_KEY and makes a live API request"]
async fn live_jev_selects_relevant_capabilities() {
    let (scored, selection) = decide("帮我画一张这个服务的架构图，并把结果发邮件给我").await;
    println!("diagram+mail: {scored:?} -> {selection:?}");
    assert!(selection.skills.contains(&"drawio".to_string()));
    assert!(selection.skills.contains(&"qq-mail".to_string()));

    let (scored, selection) = decide("查一下 Rust 1.90 最新的发布说明").await;
    println!("web: {scored:?} -> {selection:?}");
    assert!(selection.tools.contains(&"web_search".to_string()));

    let (scored, selection) = decide("你好，解释一下什么是闭包").await;
    println!("chat: {scored:?} -> {selection:?}");
    assert!(selection.is_empty());
}

#[tokio::test]
#[ignore = "requires TYPESAFE_API_KEY and makes a live API request"]
async fn live_jev_audit_allows_requested_operation() {
    let connection = AppConfig::default().jev_connection().unwrap();
    let client = JevClient::new(&connection, 10).unwrap();
    let facts = audit::AuditFacts {
        tool: "run_command",
        arguments_json: r#"{"command":"echo hello"}"#,
        context: "[user] Print hello using echo hello in the workspace.",
        workdir: "/workspace",
        policy: crate::prompts::AUTO_AUDIT_SYSTEM_PROMPT,
        sandbox: None,
    };
    let verdict = audit::review(&client, &facts, &JevAuditConfig::default())
        .await
        .unwrap();
    println!("audit: {verdict:?}");
    assert!(!matches!(verdict, audit::AuditVerdict::Deny(_)));
}

#[tokio::test]
#[ignore = "requires TYPESAFE_API_KEY and makes a live API request"]
async fn live_jev_probe_succeeds() {
    let connection = AppConfig::default().jev_connection().unwrap();
    let report = probe::probe(&connection).await;
    println!("probe: {report:?}");
    assert!(report.ok, "{}", report.detail);
}
