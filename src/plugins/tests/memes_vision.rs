use super::{memes_models::MemeModels, memes_support::*};
use serde_json::{json, Value};
use std::sync::{atomic::Ordering, Arc};

/// 【表情视觉测试】【完整流程】实际 Lua 发送原图片字节，解析带围栏 JSON 并保留旧 save 语义
/// @returns 无；元数据、提示词和图片保存均正确
#[tokio::test]
async fn memes_vision_uses_the_original_prompt_and_image_bytes() {
    let root = tempfile::tempdir().unwrap();
    let args = addition(root.path(), 1);
    let runtime = runtime(root.path(), json!({}), MemeHost::new(root.path()), "");
    let model = Arc::new(MemeModels::default());
    *model.response.lock().unwrap() = format!(
        "```json\n{}\n```",
        json!({"save":false,"name":{"zh":"视觉名称","en":"vision"},"description":"图片","usage":"聊天","avoid":"","tags":[" A ",2]})
    );
    let mut invocation = context(root.path());
    invocation.services = Some(model.clone());
    let result: Value = serde_json::from_str(
        &runtime
            .call_tool("add_meme", json!({"image":args["image"]}), invocation)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(result["metadata"]["name"]["zh"], "视觉名称");
    assert_eq!(result["metadata"]["tags"], json!(["A"]));
    let requests = model.vision_requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].1, b"image 1");
    assert_eq!(requests[0].0.mime_type, "image/png");
    assert_eq!(
        requests[0].0.system,
        "请基于图片内容回答，不要编造看不见的信息。"
    );
    assert!(requests[0].0.prompt.starts_with("# 表情包元数据生成"));
}

/// 【表情视觉测试】【故障不落图片】视觉请求或 JSON 解析失败返回补充信息要求，字段不完整报错
/// @returns 无；三种失败均不创建图片或索引
#[tokio::test]
async fn memes_vision_failures_leave_no_orphan_images() {
    for (text, failure, incomplete) in [
        ("", true, false),
        ("invalid JSON", false, false),
        ("{}", false, true),
    ] {
        let root = tempfile::tempdir().unwrap();
        let args = addition(root.path(), 1);
        let runtime = runtime(root.path(), json!({}), MemeHost::new(root.path()), "");
        let model = Arc::new(MemeModels::default());
        *model.response.lock().unwrap() = text.into();
        model.fail.store(failure, Ordering::SeqCst);
        let mut invocation = context(root.path());
        invocation.services = Some(model);
        let result = runtime
            .call_tool("add_meme", json!({"image":args["image"]}), invocation)
            .await;
        if incomplete {
            assert!(result.is_err());
        } else {
            assert_eq!(
                serde_json::from_str::<Value>(&result.unwrap()).unwrap()["needs_user_info"],
                true
            );
        }
        assert!(!root.path().join("user").exists());
    }
}
