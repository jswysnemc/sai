use super::*;
use crate::gateways::channel_context::{save_latest_channel_context, ChannelContext};
use crate::gateways::weixin_bot::client::WeixinBotClient;
use axum::{extract::State, http::HeaderMap, routing::post, Json, Router};
use tokio::sync::mpsc;

/// 【渠道消息】【测试服务】捕获参数 headers/body 并发送至 sender，返回微信成功响应。
async fn capture_message(
    State(sender): State<mpsc::UnboundedSender<(HeaderMap, Value)>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    sender.send((headers, body)).unwrap();
    Json(json!({"ret": 0}))
}

/// 【渠道消息】【测试服务】保存本地接口地址和接收队列；析构时终止服务。
struct MockWeixin {
    url: String,
    received: mpsc::UnboundedReceiver<(HeaderMap, Value)>,
    task: tokio::task::JoinHandle<()>,
}

impl MockWeixin {
    /// 【渠道消息】【测试服务】无参数；返回使用随机端口的隔离微信服务。
    async fn start() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (sender, received) = mpsc::unbounded_channel();
        let app = Router::new()
            .route("/ilink/bot/sendmessage", post(capture_message))
            .with_state(sender);
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self {
            url,
            received,
            task,
        }
    }

    /// 【渠道消息】【测试配置】无参数；返回只连接本地服务的应用配置。
    fn config(&self) -> AppConfig {
        let mut config = AppConfig::default();
        config.gateways.weixin.base_url = self.url.clone();
        config.gateways.weixin.token = "test-token".into();
        config
    }

    /// 【渠道消息】【测试接收】无参数；返回请求头及正文，超时直接报告测试失败。
    async fn request(&mut self) -> (HeaderMap, Value) {
        tokio::time::timeout(std::time::Duration::from_secs(2), self.received.recv())
            .await
            .unwrap()
            .unwrap()
    }
}

impl Drop for MockWeixin {
    /// 【渠道消息】【测试清理】无参数，返回无；避免服务任务跨测试继续运行。
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// 【渠道消息】【缺失目标回归】无参数，返回无；普通会话没有上下文时提供可操作错误。
#[tokio::test]
async fn unbound_session_reports_missing_weixin_context() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let registry = crate::tools::builtin_registry_without_mcp(&AppConfig::default(), &paths);
    assert!(registry.contains("send_channel_message"));
    let error = registry
        .call("send_channel_message", r#"{"kind":"text","text":"hello"}"#)
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains(t("no recent channel target", "没有最近使用的渠道目标")));
    assert!(error.to_string().contains(t(
        "Send a message to the Sai bot",
        "请先在该渠道向 Sai 机器人"
    )));
}

/// 【渠道消息】【主动发送回归】无参数，返回无；普通注册表与克隆在调用时读取最新接收方。
#[tokio::test]
async fn ordinary_and_cloned_registries_send_to_latest_weixin_context() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut mock = MockWeixin::start().await;
    let config = mock.config();
    let registry = crate::tools::builtin_registry_without_mcp(&config, &paths);
    assert_eq!(
        registry.permission("send_channel_message").unwrap(),
        crate::tools::ToolPermission::Writes
    );
    let inherited = registry.clone_filtered(&["send_channel_message"]);
    for (channel, user) in [
        ("current", "first-user"),
        ("default", "latest-user"),
        ("weixin", "explicit-user"),
    ] {
        save_latest_channel_context(
            &paths,
            &ChannelContext::weixin(user.into(), Some("latest-context".into())),
        )
        .unwrap();
        let output = inherited
            .call(
                "send_channel_message",
                &json!({"kind":"text", "text":"progress", "channel":channel}).to_string(),
            )
            .await
            .unwrap();
        assert_eq!(serde_json::from_str::<Value>(&output).unwrap()["ok"], true);
        let (headers, body) = mock.request().await;
        assert_eq!(headers["authorization"], "Bearer test-token");
        assert_eq!(body["msg"]["to_user_id"], user);
        assert_eq!(body["msg"]["context_token"], "latest-context");
        assert_eq!(body["msg"]["item_list"][0]["text_item"]["text"], "progress");
    }
}

/// 【渠道消息】【绑定隔离回归】无参数，返回无；通用挂载不能覆盖网关的当前用户。
#[tokio::test]
async fn generic_registration_keeps_the_bound_inbound_recipient() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut mock = MockWeixin::start().await;
    let config = mock.config();
    save_latest_channel_context(
        &paths,
        &ChannelContext::weixin("other-user".into(), Some("other-token".into())),
    )
    .unwrap();
    let mut registry = ToolRegistry::new();
    register_channel_message_tool(
        &mut registry,
        paths.clone(),
        config.clone(),
        ActiveChannelTarget::Weixin {
            client: WeixinBotClient::new(
                mock.url.clone(),
                mock.url.clone(),
                "test-token".into(),
                None,
                false,
            ),
            to_user_id: "inbound-user".into(),
            context_token: Some("inbound-token".into()),
        },
    );
    register_unbound_channel_message_tool(&mut registry, &paths, &config);
    for channel in ["current", "default", "weixin"] {
        registry
            .call(
                "send_channel_message",
                &json!({"kind":"text", "text":"reply", "channel":channel}).to_string(),
            )
            .await
            .unwrap();
        let (_, body) = mock.request().await;
        assert_eq!(body["msg"]["to_user_id"], "inbound-user");
        assert_eq!(body["msg"]["context_token"], "inbound-token");
    }
}

/// 【渠道消息】【默认可用回归】无参数，返回无；默认代码档案可用，独占白名单仍有最终决定权。
#[test]
fn default_code_profile_keeps_sender_but_exclusive_profile_can_hide_it() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = crate::config::apply_agent_override(
        AppConfig::default(),
        None,
        crate::config::AgentSurface::Tui,
    )
    .unwrap();
    let registry = crate::tools::builtin_registry_without_mcp(&config, &paths);
    let selected = crate::runner::apply_enabled_tools_filter(
        registry.clone(),
        &config,
        crate::runner::SubmissionSource::Repl,
    )
    .unwrap();
    assert!(selected.contains("send_channel_message"));
    let mut exclusive = config;
    exclusive.agent_runtime = Some(crate::config::AgentRuntimeOverride {
        enabled_tools: vec!["read_file".into()],
        exclusive: true,
        ..Default::default()
    });
    let selected = crate::runner::apply_enabled_tools_filter(
        registry,
        &exclusive,
        crate::runner::SubmissionSource::Repl,
    )
    .unwrap();
    assert!(!selected.contains("send_channel_message"));
}

/// 【渠道消息】【权限回归】无参数，返回无；交互入口挂载工具但规划权限禁止主动发送。
#[tokio::test]
async fn interactive_sender_obeys_plan_permissions() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    let mut registry = ToolRegistry::new();
    crate::tools::register_interactive_tools(
        &mut registry,
        &config,
        &paths,
        root.path().display().to_string(),
        "local".into(),
    );
    assert!(registry.contains("send_channel_message"));
    registry.set_permission_profile(crate::permission::PermissionProfile::new(
        crate::permission::PermissionProfileMode::Plan,
        root.path().to_path_buf(),
        None,
    ));
    let error = registry
        .call("send_channel_message", r#"{"kind":"text","text":"hi"}"#)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("Plan mode blocked"), "{error}");
}
