use super::{example_support, support::FixtureHost};
use sai_plugin_runtime::{
    host::PluginHost, InvocationContext, PermissionAuditDecision, PermissionAuditInput,
    PluginRuntime,
};
use serde_json::{json, Value};
use std::sync::Arc;

/// 【Jev测试】【配置样本】返回不含真实凭据的官方设置，无参数
fn settings() -> Value {
    json!({"provider":{"id":"typesafe","base_url":"https://api.typesafe.ai/v1","model":"jev-latest","api_key":"fixture-only"}})
}

/// 【Jev测试】【实例加载】host 为网络宿主，settings 为设置；返回实际 Lua 插件
fn runtime(host: Arc<dyn PluginHost>, settings: Value) -> PluginRuntime {
    let package = example_support::package("jev-audit");
    let grants = package.manifest.capabilities.clone();
    PluginRuntime::load(package, settings, grants, host).unwrap()
}

/// 【Jev测试】【调用样本】plugin 为实例；返回对合成工具请求的审核结果
async fn review(plugin: &PluginRuntime) -> sai_plugin_runtime::PermissionAuditOutput {
    plugin
        .review_permission(
            PermissionAuditInput {
                tool: "shell".into(),
                arguments: json!({"command":"echo hello"}),
                context: "[user] Print hello using echo hello in the workspace.".into(),
                policy: crate::prompts::AUTO_AUDIT_SYSTEM_PROMPT.into(),
            },
            InvocationContext {
                session_id: "fixture".into(),
                operation_id: "request-1".into(),
                workdir: "/workspace".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap()
}

/// 【Jev测试】【响应样本】choice/probabilities/confidence 为官方判断字段；返回 JSON 正文
fn response(choice: &str, probabilities: Value, confidence: f64) -> String {
    json!({"model":"jev-latest","answers":{"permission":{"type":"choice","choice":choice,"probabilities":probabilities,"confidence":confidence}}}).to_string()
}

/// 使用官方结构化问题、选定模型和宿主凭据，网络调用保持只读授权
#[tokio::test]
async fn jev_audit_uses_selected_official_provider() {
    let response = response(
        "allow",
        json!({"allow":0.98,"deny":0.01,"abstain":0.01}),
        0.95,
    );
    let host = Arc::new(FixtureHost::new(&[(200, &response)]));
    assert_eq!(
        review(&runtime(host.clone(), settings())).await.decision,
        PermissionAuditDecision::Allow
    );
    let requests = host.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(request.url, "https://api.typesafe.ai/v1/systemone");
    assert_eq!(request.method, "POST");
    assert_eq!(request.headers["Authorization"], "Bearer fixture-only");
    assert_eq!(request.timeout_ms, 10000);
    let body: Value = serde_json::from_str(request.body.as_deref().unwrap()).unwrap();
    assert_eq!(body["model"], "jev-latest");
    assert!(body.get("messages").is_none());
    assert!(body.get("tools").is_none());
    let arguments: Value =
        serde_json::from_str(body["state"]["arguments_json"].as_str().unwrap()).unwrap();
    assert_eq!(arguments["command"], "echo hello");
    assert_eq!(body["state"]["workdir"], "/workspace");
    assert_eq!(body["questions"]["permission"]["type"], "choice");
    assert_eq!(
        body["questions"]["permission"]["criteria"]
            .as_object()
            .unwrap()
            .len(),
        3
    );
    assert!(!body.to_string().contains("fixture-only"));
}

/// 明确拒绝生效，低概率、低置信度及不完整分布不能误判为允许
#[tokio::test]
async fn jev_audit_validates_distribution_and_uncertainty() {
    for (choice, distribution, confidence, expected) in [
        (
            "deny",
            json!({"allow":0.01,"deny":0.98,"abstain":0.01}),
            0.95,
            PermissionAuditDecision::Deny,
        ),
        (
            "abstain",
            json!({"allow":0.01,"deny":0.01,"abstain":0.98}),
            0.95,
            PermissionAuditDecision::Abstain,
        ),
        (
            "allow",
            json!({"allow":0.8,"deny":0.1,"abstain":0.1}),
            0.9,
            PermissionAuditDecision::Abstain,
        ),
        (
            "allow",
            json!({"allow":0.98,"deny":0.01,"abstain":0.01}),
            0.5,
            PermissionAuditDecision::Abstain,
        ),
        (
            "allow",
            json!({"allow":0.98}),
            0.95,
            PermissionAuditDecision::Abstain,
        ),
        (
            "allow",
            json!({"allow":1.2,"deny":-0.1,"abstain":-0.1}),
            0.95,
            PermissionAuditDecision::Abstain,
        ),
        (
            "allow",
            json!({"allow":0.98,"deny":0.01,"abstain":0.51}),
            0.95,
            PermissionAuditDecision::Abstain,
        ),
        (
            "allow",
            json!({"allow":0.01,"deny":0.98,"abstain":0.01}),
            0.95,
            PermissionAuditDecision::Abstain,
        ),
        (
            "ALLOW",
            json!({"allow":0.98,"deny":0.01,"abstain":0.01}),
            0.95,
            PermissionAuditDecision::Abstain,
        ),
    ] {
        let response = response(choice, distribution, confidence);
        let host = Arc::new(FixtureHost::new(&[(200, &response)]));
        assert_eq!(
            review(&runtime(host, settings())).await.decision,
            expected,
            "{response}"
        );
    }
}

/// 服务失败或非结构化响应只交还人工，错误正文不进入审核说明
#[tokio::test]
async fn jev_audit_abstains_on_transport_and_response_failures() {
    for (status, response) in [
        (200, "not JSON"),
        (200, "{}"),
        (200, "\"ALLOW\""),
        (401, "fixture-secret"),
        (429, "fixture-secret"),
        (500, "fixture-secret"),
    ] {
        let host = Arc::new(FixtureHost::new(&[(status, response)]));
        let result = review(&runtime(host, settings())).await;
        assert_eq!(result.decision, PermissionAuditDecision::Abstain);
        assert!(!result.reason.unwrap_or_default().contains("fixture-secret"));
    }
}

/// 缺少凭据时不发送匿名审核请求，也不借用其他供应商的密钥
#[tokio::test]
async fn jev_audit_missing_credentials_never_calls_provider() {
    let host = Arc::new(FixtureHost::default());
    let result = review(&runtime(host.clone(), json!({}))).await;
    assert_eq!(result.decision, PermissionAuditDecision::Abstain);
    assert!(host.requests.lock().unwrap().is_empty());
}

/// 【Jev测试】【官方联调】仅显式运行时读取现有密钥并发送无副作用的合成审核
/// 返回无；使用真实 Lua、HTTP 宿主和官方概率响应验证整条路径
#[tokio::test]
#[ignore = "requires TypeSafe credentials and makes a live API request"]
async fn jev_audit_live_official() {
    let key = std::env::var("TYPESAFE_KEY")
        .or_else(|_| std::env::var("TYPESAFE_API_KEY"))
        .expect("TypeSafe credential must be configured");
    let mut config = settings();
    config["provider"]["api_key"] = json!(key);
    let plugin = runtime(Arc::new(crate::plugins::host::SaiPluginHost), config);
    assert_eq!(
        review(&plugin).await.decision,
        PermissionAuditDecision::Allow
    );
    let denied = plugin
        .review_permission(
            PermissionAuditInput {
                tool: "shell".into(),
                arguments: json!({"command":"rm -rf /workspace/project"}),
                context:
                    "[user] Only list the files in this project. Do not delete or modify anything."
                        .into(),
                policy: crate::prompts::AUTO_AUDIT_SYSTEM_PROMPT.into(),
            },
            InvocationContext {
                session_id: "fixture".into(),
                operation_id: "request-2".into(),
                workdir: "/workspace".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(denied.decision, PermissionAuditDecision::Deny);
}

/// 独立审核授权开关可单独撤销，不与全量授权或相反开关混用
#[test]
fn jev_audit_cli_grants_are_explicit() {
    use clap::Parser;
    for flags in [
        vec!["--allow-permission-audit"],
        vec!["--no-permission-audit"],
        vec!["--allow-permission-audit", "--no-model"],
    ] {
        let mut args = vec!["sai", "plugins", "enable", "jev-audit"];
        args.extend(flags);
        assert!(crate::cli::Cli::try_parse_from(args).is_ok());
    }
    for flags in [
        vec!["--allow-permission-audit", "--no-permission-audit"],
        vec!["--grant-declared", "--allow-permission-audit"],
        vec!["--grant-declared", "--no-permission-audit"],
    ] {
        let mut args = vec!["sai", "plugins", "enable", "jev-audit"];
        args.extend(flags);
        assert!(crate::cli::Cli::try_parse_from(args).is_err());
    }
}
