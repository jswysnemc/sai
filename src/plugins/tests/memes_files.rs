use super::memes_support::*;
use serde_json::{json, Value};
use std::sync::atomic::Ordering;

/// 【表情文件测试】【六工具基础】覆盖新增、重复、搜索、显示、更新、最近记录和永久删除
/// @returns 无；元数据及原图片字节均保持正确
#[tokio::test]
async fn memes_six_tools_use_real_files() {
    let root = tempfile::tempdir().unwrap();
    let host = MemeHost::new(root.path());
    let runtime = runtime(root.path(), json!({}), host.clone(), "");
    let args = addition(root.path(), 1);
    let added = call(&runtime, root.path(), "add_meme", args.clone()).await;
    let id = added["id"].as_str().unwrap();
    assert_eq!(
        std::fs::read(added["path"].as_str().unwrap()).unwrap(),
        b"image 1"
    );
    let duplicate = call(&runtime, root.path(), "add_meme", args).await;
    assert_eq!(duplicate["already_exists"], true);
    let search = call(&runtime, root.path(), "search_meme", json!({})).await;
    assert_eq!(search["results"][0]["id"], id);
    let shown = call(
        &runtime,
        root.path(),
        "show_meme",
        json!({"id":id,"width":40,"height":15}),
    )
    .await;
    assert_eq!(shown["animation_note"], Value::Null);
    assert_eq!(host.displays.lock().unwrap()[0].1.as_deref(), Some("40x15"));
    let updated = call(
        &runtime,
        root.path(),
        "update_meme",
        json!({"id":id,"name_zh":"新名字"}),
    )
    .await;
    assert_eq!(updated["metadata"]["name"]["zh"], "新名字");
    assert_eq!(
        call(&runtime, root.path(), "recent_meme", json!({})).await["success"],
        false
    );
    assert_eq!(
        call(
            &runtime,
            root.path(),
            "delete_meme",
            json!({"id":id,"hard_delete":true})
        )
        .await["action"],
        "deleted_user_meme"
    );
    assert!(!std::path::Path::new(added["path"].as_str().unwrap()).exists());
    assert!(
        call(&runtime, root.path(), "search_meme", json!({})).await["results"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

/// 【表情文件测试】【内置覆盖】修改元数据保留内置图片，禁用后可以重新启用
/// @returns 无；删除内置覆盖仅禁用，不触碰发布图片
#[tokio::test]
async fn memes_builtin_overlays_keep_images_and_can_be_reenabled() {
    let root = tempfile::tempdir().unwrap();
    seed(
        root.path(),
        "builtin",
        vec![item("sha256:abcdef", "images/base.png", "原名")],
    );
    let host = MemeHost::new(root.path());
    let runtime = runtime(root.path(), json!({}), host.clone(), "");
    call(
        &runtime,
        root.path(),
        "update_meme",
        json!({"id":"abc","name_zh":"覆盖名"}),
    )
    .await;
    call(&runtime, root.path(), "show_meme", json!({"id":"abc"})).await;
    assert!(host.displays.lock().unwrap()[0]
        .0
        .ends_with("builtin/sai/images/base.png"));
    call(
        &runtime,
        root.path(),
        "update_meme",
        json!({"id":"abc","enabled":false}),
    )
    .await;
    assert!(
        call(&runtime, root.path(), "search_meme", json!({})).await["results"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    call(
        &runtime,
        root.path(),
        "update_meme",
        json!({"id":"abc","enabled":true}),
    )
    .await;
    assert_eq!(
        call(&runtime, root.path(), "search_meme", json!({})).await["results"][0]["name"]["zh"],
        "覆盖名"
    );
    assert_eq!(
        call(
            &runtime,
            root.path(),
            "delete_meme",
            json!({"id":"abc","hard_delete":true})
        )
        .await["action"],
        "disabled_builtin_meme"
    );
    assert!(root.path().join("builtin/sai/images/base.png").is_file());
}

/// 【表情文件测试】【元数据失败】元数据验证必须先于新文件创建
/// @returns 无；失败不产生用户索引或孤立图片
#[tokio::test]
async fn memes_invalid_metadata_creates_no_image() {
    let root = tempfile::tempdir().unwrap();
    let runtime = runtime(root.path(), json!({}), MemeHost::new(root.path()), "");
    let mut args = addition(root.path(), 1);
    args["usage"] = json!("");
    assert!(runtime
        .call_tool("add_meme", args, context(root.path()))
        .await
        .is_err());
    assert!(!root.path().join("user").exists());
}

/// 【表情文件测试】【删除恢复】普通错误保留可重试记录，不把回收失败静默改为永久删除
/// @returns 无；显式重新选择永久删除后才移除图片
#[tokio::test]
async fn memes_failed_deletion_requires_an_explicit_retry() {
    let root = tempfile::tempdir().unwrap();
    let host = MemeHost::new(root.path());
    let runtime = runtime(root.path(), json!({}), host.clone(), "");
    let added = call(&runtime, root.path(), "add_meme", addition(root.path(), 1)).await;
    host.fail_removal.store(true, Ordering::SeqCst);
    assert!(runtime
        .call_tool(
            "delete_meme",
            json!({"id":added["id"]}),
            context(root.path())
        )
        .await
        .is_err());
    assert!(std::path::Path::new(added["path"].as_str().unwrap()).is_file());
    assert!(
        call(&runtime, root.path(), "search_meme", json!({})).await["results"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(runtime
        .call_tool(
            "update_meme",
            json!({"id":added["id"],"enabled":true}),
            context(root.path())
        )
        .await
        .is_err());
    host.fail_removal.store(false, Ordering::SeqCst);
    call(
        &runtime,
        root.path(),
        "delete_meme",
        json!({"id":added["id"],"hard_delete":true}),
    )
    .await;
    assert!(!std::path::Path::new(added["path"].as_str().unwrap()).exists());
}

/// 【表情文件测试】【禁用后新增】同内容重新加入时恢复可见状态并清理旧图片
/// @returns 无；新的完整元数据生效且用户库只有一个引用文件
#[tokio::test]
async fn memes_readding_disabled_content_does_not_leave_the_previous_image() {
    let root = tempfile::tempdir().unwrap();
    let runtime = runtime(root.path(), json!({}), MemeHost::new(root.path()), "");
    let args = addition(root.path(), 1);
    let first = call(&runtime, root.path(), "add_meme", args.clone()).await;
    call(
        &runtime,
        root.path(),
        "update_meme",
        json!({"id":first["id"],"enabled":false}),
    )
    .await;
    let mut next = args;
    next["name_zh"] = json!("重新添加");
    let added = call(&runtime, root.path(), "add_meme", next).await;
    assert_eq!(added["name"]["zh"], "重新添加");
    assert_eq!(
        std::fs::read_dir(root.path().join("user/sai/images"))
            .unwrap()
            .count(),
        1
    );
    assert!(!std::path::Path::new(first["path"].as_str().unwrap()).exists());
}

/// 【表情文件测试】【替换清理失败】新索引发布后清理失败，结果明确指出未引用的旧文件
/// @returns 无；新图片仍可读取，旧图片保留且内部清理字段不会进入公开结果
#[tokio::test]
async fn memes_readding_reports_cleanup_failure_after_successful_publication() {
    let root = tempfile::tempdir().unwrap();
    let host = MemeHost::new(root.path());
    let runtime = runtime(root.path(), json!({}), host.clone(), "");
    let args = addition(root.path(), 1);
    let first = call(&runtime, root.path(), "add_meme", args.clone()).await;
    call(
        &runtime,
        root.path(),
        "update_meme",
        json!({"id":first["id"],"enabled":false}),
    )
    .await;
    host.fail_removal.store(true, Ordering::SeqCst);
    let added = call(&runtime, root.path(), "add_meme", args).await;
    assert_eq!(added["success"], true);
    assert_eq!(added["unreferenced_path"], first["path"]);
    assert!(added["cleanup_error"]
        .as_str()
        .unwrap()
        .contains("fixture removal failure"));
    assert!(added.get("_retired").is_none());
    assert!(std::path::Path::new(first["path"].as_str().unwrap()).is_file());
    call(
        &runtime,
        root.path(),
        "show_meme",
        json!({"id":added["id"]}),
    )
    .await;
    assert_eq!(host.displays.lock().unwrap()[0].0, added["path"]);
}
