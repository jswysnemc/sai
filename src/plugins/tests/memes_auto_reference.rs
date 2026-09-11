use super::{memes_models::MemeModels, memes_support::*};
use serde_json::{json, Value};
use std::sync::Arc;

/// 【表情自动对照】【模型消息】比较完整提示前缀及候选 JSON，忽略对象键顺序
/// @param actual 实际请求；expected 为冻结原版消息
/// @returns 无；语气规则、Unicode 截断和候选字段必须一致
fn compare_messages(actual: &sai_plugin_runtime::host::ModelRequest, expected: &Value) {
    assert_eq!(actual.messages.len(), 2);
    assert_eq!(actual.messages[0].content, expected[0]["content"]);
    let marker = "\n\n候选表情：";
    let (prefix, candidates) = actual.messages[1].content.split_once(marker).unwrap();
    let (old_prefix, old_candidates) = expected[1]["content"]
        .as_str()
        .unwrap()
        .split_once(marker)
        .unwrap();
    assert_eq!(prefix, old_prefix);
    assert_eq!(
        serde_json::from_str::<Value>(candidates).unwrap(),
        serde_json::from_str::<Value>(old_candidates).unwrap()
    );
    assert!(actual.tools.is_empty());
}

/// 【表情自动对照】【原版矩阵】使用实际 Lua 策略覆盖概率、置信度、前缀、损坏模型输出和空库
/// @returns 无；决策、提醒、模型请求及成功记录都与原版对照
#[tokio::test]
async fn memes_auto_policy_matches_frozen_native_decisions_and_prompts() {
    let fixtures: Value = serde_json::from_str(include_str!("fixtures/memes_auto.json")).unwrap();
    assert_eq!(
        fixtures["source_commit"],
        "90a911c8859d1bef72e7d983e865a815a0abab47"
    );
    for case in fixtures["cases"].as_array().unwrap() {
        let root = tempfile::tempdir().unwrap();
        seed(root.path(), "builtin", vec![fixtures["item"].clone()]);
        if !case["disabled_ids"].is_null() {
            seed(root.path(), "user", vec![]);
            std::fs::write(
                root.path().join("user/sai/index.json"),
                json!({"memes":[],"disabled_ids":case["disabled_ids"]}).to_string(),
            )
            .unwrap();
        }
        let host = MemeHost::new(root.path());
        let suffix = format!(
            "\nmath.random=function()return {} end\nsai.time.now=function()return 1700000000 end\n",
            case["sample"]
        );
        let runtime = runtime(root.path(), case["config"].clone(), host.clone(), &suffix);
        let model = Arc::new(MemeModels::default());
        *model.response.lock().unwrap() = case["response"].as_str().unwrap().into();
        let mut invocation = context(root.path());
        invocation.services = Some(model.clone());
        let plan = runtime
            .prepare_reply(case["user"].as_str().unwrap(), invocation.clone())
            .await
            .unwrap_or_else(|error| panic!("{}: {error:#}", case["label"]));
        assert!(plan.context.is_none());
        assert_eq!(
            plan.has_delivery(),
            !case["expected"].is_null(),
            "{}",
            case["label"]
        );
        assert_eq!(
            plan.reminder.as_deref(),
            case["expected"]["reminder"].as_str(),
            "{}",
            case["label"]
        );
        assert!(host.displays.lock().unwrap().is_empty());
        assert!(!root.path().join("recent").exists());
        let requests = model.requests.lock().unwrap();
        let expected = case["model_requests"].as_array().unwrap();
        assert_eq!(requests.len(), expected.len(), "{}", case["label"]);
        for (actual, old) in requests.iter().zip(expected) {
            compare_messages(actual, old);
        }
        drop(requests);
        if plan.has_delivery() {
            runtime.complete_reply(plan, invocation).await.unwrap();
            assert_eq!(
                call(&runtime, root.path(), "recent_meme", json!({})).await["recent"],
                case["expected"]["event"],
                "{}",
                case["label"]
            );
            assert_eq!(host.displays.lock().unwrap().len(), 1);
        }
    }
}

/// 【表情自动发送测试】【完成前复核】准备后禁用或删除条目，原计划不能继续显示或保存发送记录
/// @returns 无；业务变更通过真实 Lua 工具提交，完成阶段拒绝过期条目
#[tokio::test]
async fn memes_reply_rechecks_disabled_and_deleted_entries() {
    for action in ["update_meme", "delete_meme"] {
        let root = tempfile::tempdir().unwrap();
        seed(
            root.path(),
            "user",
            vec![item("sha256:abcdef", "images/base.png", "企鹅")],
        );
        let host = MemeHost::new(root.path());
        let runtime = runtime(
            root.path(),
            json!({"auto_send_enabled":true,"auto_send_probability":1.0}),
            host.clone(),
            "",
        );
        let model = Arc::new(MemeModels::default());
        *model.response.lock().unwrap() = r#"{"send":true,"id":"abc","confidence":1}"#.into();
        let mut invocation = context(root.path());
        invocation.services = Some(model);
        let prepared = runtime
            .prepare_reply("Linux", invocation.clone())
            .await
            .unwrap();
        assert!(prepared.has_delivery());
        let arguments = if action == "update_meme" {
            json!({"id":"abc","enabled":false})
        } else {
            json!({"id":"abc","hard_delete":true})
        };
        call(&runtime, root.path(), action, arguments).await;
        assert!(runtime.complete_reply(prepared, invocation).await.is_err());
        assert!(host.displays.lock().unwrap().is_empty());
        assert!(!root.path().join("recent").exists());
    }
}
