use super::{AutoAuditBackend, PermissionAllowSource, PermissionDecision};
use crate::{config::AppConfig, paths::SaiPaths, plugins};
use serde_json::json;
use std::path::Path;

/// 【审核分发测试】【安装样本】source 为 Lua 审核代码；返回隔离路径及配置
fn fixture(source: &str) -> (tempfile::TempDir, SaiPaths, AppConfig) {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let directory = paths.config_dir.join("plugins/audit-fixture");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("sai-plugin.json"), json!({
        "api_version":1,"id":"audit-fixture","version":"1.0.0","name":"Audit fixture",
        "description":"Audit dispatch fixture","entry":"init.lua","capabilities":{"permission_audit":true},
    }).to_string()).unwrap();
    std::fs::write(directory.join("init.lua"), source).unwrap();
    let mut config = AppConfig::default();
    config.permission.auto_audit_plugin_id = "audit-fixture".into();
    plugins::set_enabled(
        &config,
        &paths,
        "audit-fixture",
        true,
        plugins::GrantUpdate::Declared,
    )
    .unwrap();
    (root, paths, config)
}

/// 明确选择插件时不调用聊天模型，审核决定沿用自动审核来源
#[tokio::test]
async fn permission_audit_plugin_submits_once_without_llm_client() {
    let (_root,paths,config)=fixture("sai.register_permission_audit({review=function(input,ctx) assert(input.tool=='shell'); return {decision='allow',reason='fixture'} end})");
    let backend = AutoAuditBackend::resolve(&config, &paths).unwrap();
    assert!(matches!(backend, AutoAuditBackend::Plugin(_)));
    let (request, receiver) =
        super::request_permission_with_auto_audit("fixture", "shell", "{}", true);
    assert!(backend
        .run(&request, "explicit request", Path::new("/workspace"))
        .await
        .unwrap());
    assert!(matches!(
        receiver.await.unwrap(),
        PermissionDecision::Allow {
            source: PermissionAllowSource::AutoAudit,
            ..
        }
    ));
    assert!(!backend
        .run(&request, "explicit request", Path::new("/workspace"))
        .await
        .unwrap());
}

/// 人工先作决定时，晚到的自动允许不能覆盖拒绝
#[tokio::test]
async fn permission_audit_preserves_human_decision() {
    let (_root, paths, config) =
        fixture("sai.register_permission_audit({review=function() return {decision='allow'} end})");
    let backend = AutoAuditBackend::resolve(&config, &paths).unwrap();
    let (request, receiver) =
        super::request_permission_with_auto_audit("fixture", "shell", "{}", true);
    super::decide_permission(
        &request.id,
        PermissionDecision::Deny {
            reply: Some("human decision".into()),
        },
    )
    .unwrap();
    assert!(!backend
        .run(&request, "context", Path::new("/workspace"))
        .await
        .unwrap());
    assert!(matches!(
        receiver.await.unwrap(),
        PermissionDecision::Deny { .. }
    ));
}

/// 插件弃权或执行错误继续等待人工；缺失、停用或撤权时不退回其他自动审核模型
#[tokio::test]
async fn permission_audit_failure_never_falls_back_to_llm_or_approval() {
    for source in [
        "sai.register_permission_audit({review=function() return {decision='abstain'} end})",
        "sai.register_permission_audit({review=function() error('fixture failure') end})",
    ] {
        let (_root, paths, config) = fixture(source);
        let backend = AutoAuditBackend::resolve(&config, &paths).unwrap();
        let (request, mut receiver) =
            super::request_permission_with_auto_audit("fixture", "shell", "{}", true);
        assert!(!backend
            .run(&request, "context", Path::new("/workspace"))
            .await
            .unwrap_or(false));
        assert!(matches!(
            receiver.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty)
        ));
        super::decide_permission(&request.id, PermissionDecision::allow_once()).unwrap();
        receiver.await.unwrap();
        plugins::set_enabled(
            &config,
            &paths,
            "audit-fixture",
            true,
            plugins::GrantUpdate::Changes(plugins::GrantChanges {
                permission_audit: Some(false),
                ..Default::default()
            }),
        )
        .unwrap();
        assert!(AutoAuditBackend::resolve(&config, &paths).is_err());
    }
    let (_root, paths, mut config) = fixture("");
    assert!(AutoAuditBackend::resolve(&config, &paths).is_err());
    config.permission.auto_audit_plugin_id = "missing-plugin".into();
    assert!(AutoAuditBackend::resolve(&config, &paths).is_err());
}

/// 审核复用显式选定的供应商凭据，不需要该供应商作为主会话模型
#[tokio::test]
async fn permission_audit_reuses_selected_provider_without_persisting_key() {
    let (_root,paths,mut config)=fixture("sai.register_permission_audit({review=function() assert(sai.config.provider.id=='typesafe'); assert(sai.config.provider.model=='jev-latest'); assert(sai.config.provider.api_key=='fixture-only'); return {decision='deny'} end})");
    let mut provider = crate::config::ProviderConfig::default_openai();
    provider.id = "typesafe".into();
    provider.base_url = "https://api.typesafe.ai/v1".into();
    provider.api_key = Some("fixture-only".into());
    config.providers.push(provider);
    config.permission.auto_audit_provider_id = "typesafe".into();
    config.permission.auto_audit_model = "jev-latest".into();
    let backend = AutoAuditBackend::resolve(&config, &paths).unwrap();
    let (request, receiver) =
        super::request_permission_with_auto_audit("fixture", "shell", "{}", true);
    assert!(backend
        .run(&request, "context", Path::new("/workspace"))
        .await
        .unwrap());
    assert!(matches!(
        receiver.await.unwrap(),
        PermissionDecision::Deny { .. }
    ));
    assert!(
        !std::fs::read_to_string(paths.config_dir.join("plugins.jsonc"))
            .unwrap()
            .contains("fixture-only")
    );
}
