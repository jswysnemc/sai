use super::memes_support::*;
use serde_json::{json, Value};
use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

/// 【表情并发测试】【独立实例】同时添加不同内容必须保留全部条目及图片
/// @returns 无；实际正式宿主的条件写入不能覆盖其他调用结果
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn memes_concurrent_additions_merge_without_lost_updates() {
    let root = tempfile::tempdir().unwrap();
    let mut tasks = Vec::new();
    for number in 0..8 {
        let host = MemeHost::new(root.path());
        let runtime = runtime(root.path(), json!({}), host, "");
        let args = addition(root.path(), number);
        let path = root.path().to_path_buf();
        tasks.push(tokio::spawn(async move {
            call(&runtime, &path, "add_meme", args).await
        }));
    }
    for task in tasks {
        assert_eq!(task.await.unwrap()["success"], true);
    }
    let bytes = std::fs::read(root.path().join("user/sai/index.json")).unwrap();
    let index: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(index["memes"].as_array().unwrap().len(), 8);
    assert_eq!(
        std::fs::read_dir(root.path().join("user/sai/images"))
            .unwrap()
            .count(),
        8
    );
}

/// 【表情并发测试】【同内容添加】多个实例同时添加同一图片，只保留一个索引引用和文件
/// @returns 无；竞争失败的独占图片得到清理
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn memes_concurrent_duplicate_additions_leave_one_image() {
    let root = tempfile::tempdir().unwrap();
    let args = addition(root.path(), 1);
    let mut tasks = Vec::new();
    for _ in 0..6 {
        let runtime = runtime(root.path(), json!({}), MemeHost::new(root.path()), "");
        let input = args.clone();
        let path = root.path().to_path_buf();
        tasks.push(tokio::spawn(async move {
            call(&runtime, &path, "add_meme", input).await
        }));
    }
    let mut created = 0;
    for task in tasks {
        if task.await.unwrap()["already_exists"] != true {
            created += 1;
        }
    }
    assert_eq!(created, 1);
    assert_eq!(
        std::fs::read_dir(root.path().join("user/sai/images"))
            .unwrap()
            .count(),
        1
    );
}

/// 【表情并发测试】【删除后取消】文件操作已完成但索引未提交时，重试只完成原记录
/// @returns 无；新建实例可以恢复，并且随后添加使用不同文件名
#[tokio::test]
async fn memes_cancelled_deletion_can_resume_without_reusing_the_old_path() {
    let root = tempfile::tempdir().unwrap();
    let host = MemeHost::new(root.path());
    let plugin = Arc::new(runtime(root.path(), json!({}), host.clone(), ""));
    let args = addition(root.path(), 1);
    let added = call(&plugin, root.path(), "add_meme", args.clone()).await;
    host.pause_after_removal.store(true, Ordering::SeqCst);
    let instance = plugin.clone();
    let invocation = context(root.path());
    let id = added["id"].clone();
    let task = tokio::spawn(async move {
        instance
            .call_tool(
                "delete_meme",
                json!({"id":id,"hard_delete":true}),
                invocation,
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(2), host.removed.notified())
        .await
        .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert!(!std::path::Path::new(added["path"].as_str().unwrap()).exists());
    let recovered = runtime(root.path(), json!({}), MemeHost::new(root.path()), "");
    assert!(
        call(&recovered, root.path(), "search_meme", json!({})).await["results"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    call(
        &recovered,
        root.path(),
        "delete_meme",
        json!({"id":added["id"]}),
    )
    .await;
    let next = call(&recovered, root.path(), "add_meme", args).await;
    assert_eq!(next["id"], added["id"]);
    assert_ne!(next["path"], added["path"]);
    let index: Value =
        serde_json::from_slice(&std::fs::read(root.path().join("user/sai/index.json")).unwrap())
            .unwrap();
    assert!(index["pending_deletions"].as_array().unwrap().is_empty());
}

/// 【表情并发测试】【禁用后竞争新增】多个实例替换同一禁用项时只发布一张新图片
/// @returns 无；旧图片和竞争失败图片均得到清理
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn memes_concurrent_readding_disabled_content_cleans_only_retired_images() {
    let root = tempfile::tempdir().unwrap();
    let original = runtime(root.path(), json!({}), MemeHost::new(root.path()), "");
    let args = addition(root.path(), 1);
    let old = call(&original, root.path(), "add_meme", args.clone()).await;
    call(
        &original,
        root.path(),
        "update_meme",
        json!({"id":old["id"],"enabled":false}),
    )
    .await;
    let mut tasks = Vec::new();
    for _ in 0..6 {
        let runtime = runtime(root.path(), json!({}), MemeHost::new(root.path()), "");
        let path = root.path().to_path_buf();
        let input = args.clone();
        tasks.push(tokio::spawn(async move {
            call(&runtime, &path, "add_meme", input).await
        }));
    }
    let mut created = 0;
    for task in tasks {
        let result = task.await.unwrap();
        assert!(result.get("cleanup_error").is_none());
        if result["already_exists"] != true {
            created += 1;
        }
    }
    assert_eq!(created, 1);
    assert!(!std::path::Path::new(old["path"].as_str().unwrap()).exists());
    assert_eq!(
        std::fs::read_dir(root.path().join("user/sai/images"))
            .unwrap()
            .count(),
        1
    );
}

/// 【表情并发测试】【新增后取消】已完成图片创建无法随异步取消回滚，索引不会引用半成品
/// @returns 无；验证未引用文件边界，下一次新增使用不同独占文件名且正常完成
#[tokio::test]
async fn memes_cancel_after_image_publication_leaves_an_unindexed_file() {
    let root = tempfile::tempdir().unwrap();
    let host = MemeHost::new(root.path());
    host.pause_after_image.store(true, Ordering::SeqCst);
    let plugin = Arc::new(runtime(root.path(), json!({}), host.clone(), ""));
    let args = addition(root.path(), 1);
    let instance = plugin.clone();
    let input = args.clone();
    let invocation = context(root.path());
    let task = tokio::spawn(async move { instance.call_tool("add_meme", input, invocation).await });
    tokio::time::timeout(Duration::from_secs(2), host.image_created.notified())
        .await
        .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert!(!root.path().join("user/sai/index.json").exists());
    let orphan = std::fs::read_dir(root.path().join("user/sai/images"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let recovered = runtime(root.path(), json!({}), MemeHost::new(root.path()), "");
    assert!(
        call(&recovered, root.path(), "search_meme", json!({})).await["results"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let added = call(&recovered, root.path(), "add_meme", args).await;
    assert_ne!(
        std::path::Path::new(added["path"].as_str().unwrap()),
        orphan
    );
    assert_eq!(std::fs::read(orphan).unwrap(), b"image 1");
    assert_eq!(
        std::fs::read_dir(root.path().join("user/sai/images"))
            .unwrap()
            .count(),
        2
    );
}
