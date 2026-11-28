use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::super_agent::SuperAgentService;
use crate::web::api;
use crate::web::app_state::WebAppState;
use crate::web::runs::RunManager;
use crate::web::services::weixin_login::WeixinLoginManager;
use crate::web::system_monitor::SystemMonitor;
use crate::web::terminal::TerminalManager;
use crate::web::workspaces::WorkspaceManager;
use reqwest::StatusCode;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;

fn test_paths(root: &Path) -> SaiPaths {
    SaiPaths {
        config_dir: root.join("config"),
        config_file: root.join("config/config.jsonc"),
        secrets_file: root.join("config/secrets.jsonc"),
        skills_dir: root.join("skills"),
        data_dir: root.join("data"),
        cache_dir: root.join("cache"),
        state_dir: root.join("state"),
        pictures_dir: root.join("pictures"),
        fish_hook_file: root.join("fish/sai.fish"),
        bash_hook_file: root.join("bash/sai.sh"),
        zsh_hook_file: root.join("zsh/sai.zsh"),
        powershell_hook_file: root.join("ps/sai.ps1"),
    }
}

async fn start_test_server(paths: &SaiPaths, ws: &Path) -> (String, tokio::task::JoinHandle<()>) {
    AppConfig::init_files(paths).unwrap();
    let state = WebAppState {
        paths: paths.clone(),
        auth_token: Arc::from("test-token"),
        password_hash: None,
        allow_anonymous: true,
        workspaces: WorkspaceManager::new_for_tests(paths, ws).unwrap(),
        runs: RunManager::new(paths).unwrap(),
        terminals: TerminalManager::new(),
        system_monitor: SystemMonitor::new(),
        weixin_login: WeixinLoginManager::new(paths),
        super_agent: SuperAgentService::open(paths).unwrap(),
    };
    let app = api::router(state.clone()).with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (format!("http://{addr}"), handle)
}

/// 【Web API 测试】【Super-Agent 端到端接口】验证群组 CRUD、群聊、渠道绑定、渠道模拟入站、Watchdog 与配置更新。
#[tokio::test]
async fn super_agent_web_api_roundtrip_works() {
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path().join("workspace");
    std::fs::create_dir_all(&ws).unwrap();
    let paths = test_paths(dir.path());
    let (base, server) = start_test_server(&paths, &ws).await;
    let client = reqwest::Client::new();

    // 1. 首次拉取概览自动创建默认群组，并验证能力与工具接口返回完整工具边界与网关状态
    let res = client
        .get(format!("{base}/api/super-agent/overview"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let overview: Value = res.json().await.unwrap();
    assert_eq!(overview["group_count"].as_u64(), Some(1));

    let caps: Value = client
        .get(format!("{base}/api/super-agent/capabilities"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(caps["leader_tools"].as_array().unwrap().len(), 9);
    assert_eq!(caps["worker_tools"].as_array().unwrap().len(), 4);
    assert!(caps["gateways"]["weixin"].is_object());

    // 2. 新建群组
    let res = client
        .post(format!("{base}/api/super-agent/groups"))
        .json(&json!({ "name": "Web Dev Squad", "description": "Leader + Runners" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let created: Value = res.json().await.unwrap();
    let group_id = created["group"]["id"].as_str().unwrap().to_string();

    // 3. 在群组中发送消息（单元测试关闭 auto_dispatch 避免调用真实外部模型）
    let res = client
        .post(format!("{base}/api/super-agent/groups/{group_id}/messages"))
        .json(&json!({ "content": "请拆解任务并安排 Runner 执行", "auto_dispatch": false }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let posted: Value = res.json().await.unwrap();
    assert_eq!(posted["route_path"].as_str(), Some("slow_path"));

    // 4. 绑定微信渠道并注入模拟渠道消息
    let res = client
        .post(format!("{base}/api/super-agent/groups/{group_id}/channels"))
        .json(&json!({ "channel": "weixin", "external_chat_id": "wx_boss", "label": "Boss Weixin" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let binding: Value = res.json().await.unwrap();
    let binding_id = binding["id"].as_str().unwrap().to_string();

    let res = client
        .post(format!(
            "{base}/api/super-agent/groups/{group_id}/channel-inbound"
        ))
        .json(&json!({
            "channel": "weixin",
            "external_chat_id": "wx_boss",
            "actor": "Boss",
            "content": "汇总今日构建结果",
            "simulate_outbound": true,
            "auto_dispatch": false
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let inbound: Value = res.json().await.unwrap();
    assert_eq!(inbound["group_id"].as_str(), Some(group_id.as_str()));

    // 5. 读取群组详情、运行 Watchdog 巡检、更新配置并解绑渠道
    let detail: Value = client
        .get(format!("{base}/api/super-agent/groups/{group_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(detail["messages"].as_array().unwrap().len() >= 3);
    assert_eq!(detail["channels"].as_array().unwrap().len(), 1);

    let wd: Value = client
        .post(format!("{base}/api/super-agent/watchdog/check"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(!wd.as_array().unwrap().is_empty());

    let mut cfg: Value = client
        .get(format!("{base}/api/super-agent/config"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    cfg["channel_leader_routing"] = Value::Bool(true);
    let saved_cfg: Value = client
        .put(format!("{base}/api/super-agent/config"))
        .json(&cfg)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(saved_cfg["channel_leader_routing"].as_bool(), Some(true));

    let unbind: Value = client
        .delete(format!(
            "{base}/api/super-agent/groups/{group_id}/channels/{binding_id}"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(unbind["removed"].as_bool(), Some(true));

    server.abort();
}
