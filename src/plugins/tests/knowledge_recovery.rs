use super::{knowledge_host::KnowledgeHost, knowledge_support::*};
use crate::config::AppConfig;
use serde_json::json;
use std::{sync::Arc, time::Duration};

/// 【知识库恢复测试】【半写入恢复】正文已提交而元数据失败时，只读拒绝并由后续管理入口恢复
/// @returns 无；恢复完成后删除日志且元数据指向完整新正文
#[tokio::test]
async fn knowledge_recovers_failed_metadata_publication_before_reading() {
    let root = tempfile::tempdir().unwrap();
    seed(root.path(), &json!({"note.md":"old\n"}), true);
    let (plugin, host) = runtime(root.path(), json!({}));
    *host.fail_publish.lock().unwrap() = Some("kb_meta.db".into());
    let error = call(
        &plugin,
        root.path(),
        "edit_knowledge_base_file",
        json!({"file_name":"note.md","start_line":1,"end_line":1,"replacement":"new"}),
        true,
    )
    .await
    .unwrap_err();
    assert!(format!("{error:#}").contains("publication failure"));
    assert_eq!(
        std::fs::read_to_string(root.path().join("kb/files/note.md")).unwrap(),
        "new\n"
    );
    assert!(root.path().join("kb/pending-write.json").exists());
    let error = call(
        &plugin,
        root.path(),
        "read_knowledge_base_file",
        json!({"file_name":"note.md"}),
        false,
    )
    .await
    .unwrap_err();
    assert!(format!("{error:#}").contains("unfinished write"));
    *host.fail_publish.lock().unwrap() = None;
    command(&plugin, root.path(), "reindex", json!({}))
        .await
        .unwrap();
    assert!(!root.path().join("kb/pending-write.json").exists());
    let output = call(
        &plugin,
        root.path(),
        "search_knowledge_base",
        json!({"query":"new"}),
        false,
    )
    .await
    .unwrap();
    assert_eq!(output["total_matches"], 1);
    let db = rusqlite::Connection::open(root.path().join("kb/kb_meta.db")).unwrap();
    let digest: String = db
        .query_row("SELECT content_sha256 FROM files", [], |row| row.get(0))
        .unwrap();
    assert_eq!(digest, super::binary_conditional_support::digest(b"new\n"));
}

/// 【知识库恢复测试】【取消回收】发布正文后的异步取消归还私有锁，恢复不依赖原 Lua 回调清理
/// @returns 无；新实例可以完成原变更且不重复创建元数据
#[tokio::test]
async fn knowledge_cancelled_write_releases_lock_and_recovers_in_a_new_runtime() {
    let root = tempfile::tempdir().unwrap();
    seed(root.path(), &json!({"note.md":"old\n"}), true);
    let (plugin, host) = runtime(root.path(), json!({}));
    *host.pause_publish.lock().unwrap() = Some("note.md".into());
    let plugin = Arc::new(plugin);
    let running = plugin.clone();
    let directory = root.path().to_path_buf();
    let task = tokio::spawn(async move {
        call(
            &running,
            &directory,
            "edit_knowledge_base_file",
            json!({"file_name":"note.md","start_line":1,"end_line":1,"replacement":"new"}),
            true,
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(3), host.published.notified())
        .await
        .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    let (fresh, _) = runtime(root.path(), json!({}));
    command(&fresh, root.path(), "reindex", json!({}))
        .await
        .unwrap();
    assert_eq!(files(root.path()), json!({"note.md":"new\n"}));
    assert!(!root.path().join("kb/pending-write.json").exists());
}

/// 【知识库恢复测试】【删除修订】中断后重新创建或修改的正文不能被旧删除记录移除
/// @returns 无；恢复原修订后才允许完成删除
#[tokio::test]
async fn knowledge_pending_removal_refuses_recreated_file_content() {
    let root = tempfile::tempdir().unwrap();
    seed(root.path(), &json!({"note.md":"old\n"}), true);
    let (plugin, host) = runtime(root.path(), json!({}));
    *host.pause_publish.lock().unwrap() = Some("pending-write.json".into());
    let directory = root.path().to_path_buf();
    let task = tokio::spawn(async move {
        call(
            &plugin,
            &directory,
            "remove_knowledge_base_file",
            json!({"file_name":"note.md"}),
            true,
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(3), host.published.notified())
        .await
        .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    std::fs::write(root.path().join("kb/files/note.md"), "external\n").unwrap();
    let (fresh, _) = runtime(root.path(), json!({}));
    let error = command(&fresh, root.path(), "reindex", json!({}))
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("changed during pending removal"));
    assert_eq!(files(root.path()), json!({"note.md":"external\n"}));
    std::fs::write(root.path().join("kb/files/note.md"), "old\n").unwrap();
    command(&fresh, root.path(), "reindex", json!({}))
        .await
        .unwrap();
    assert_eq!(files(root.path()), json!({}));
}

/// 【知识库恢复测试】【篡改记录】日志中的绝对路径、父目录和错误摘要不能进入正文或索引变更
/// @returns 无；外部文件和原正文均保持不变
#[tokio::test]
async fn knowledge_recovery_validates_journal_names_and_revisions() {
    let root = tempfile::tempdir().unwrap();
    seed(root.path(), &json!({"note.md":"old"}), true);
    let (plugin, _) = runtime(root.path(), json!({}));
    for (name, expected) in [
        ("../outside", json!(null)),
        ("note.md", json!("invalid")),
        ("/outside", json!(null)),
    ] {
        std::fs::write(root.path().join("kb/pending-write.json"),json!({"version":1,"kind":"write","name":name,"expected":expected,"content":"new","clear_semantic":false}).to_string()).unwrap();
        assert!(command(&plugin, root.path(), "reindex", json!({}))
            .await
            .is_err());
        assert_eq!(files(root.path()), json!({"note.md":"old"}));
        assert!(!root.path().join("outside").exists());
    }
}

/// 【知识库恢复测试】【实例并发】八个独立运行时共享正式宿主锁，文件和元数据不会丢失更新
/// @returns 无；所有提交完成后每个名称只保留一条记录
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn knowledge_concurrent_runtimes_preserve_all_committed_files() {
    let root = tempfile::tempdir().unwrap();
    let mut tasks = tokio::task::JoinSet::new();
    for index in 0..8 {
        let directory = root.path().to_path_buf();
        let host = KnowledgeHost::new(root.path());
        let plugin = configured(
            root.path(),
            &AppConfig::default(),
            json!({}),
            host,
            "",
            |_| {},
        );
        tasks.spawn(async move {
            call(
                &plugin,
                &directory,
                "upload_text_to_knowledge_base",
                json!({"file_name":format!("note-{index}.md"),"content":"saved"}),
                true,
            )
            .await
        });
    }
    while let Some(result) = tasks.join_next().await {
        result.unwrap().unwrap();
    }
    let (plugin, _) = runtime(root.path(), json!({}));
    let stats: serde_json::Value = serde_json::from_str(
        &command(&plugin, root.path(), "stats", json!({}))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(stats["files"], 8);
    assert_eq!(files(root.path()).as_object().unwrap().len(), 8);
    assert!(!root.path().join("kb/pending-write.json").exists());
}

/// 【知识库恢复测试】【旧结构补齐】空数据库及缺表数据库沿用幂等初始化，非法结构不能覆盖
/// @returns 无；有效旧内容保留，损坏镜像仍是原字节
#[tokio::test]
async fn knowledge_initializes_missing_tables_but_preserves_invalid_databases() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("kb/files")).unwrap();
    let path = root.path().join("kb/kb_meta.db");
    std::fs::write(&path, b"").unwrap();
    let semantic = rusqlite::Connection::open(root.path().join("kb/semantic_index.db")).unwrap();
    semantic
        .execute_batch("CREATE TABLE keep(value TEXT); INSERT INTO keep VALUES ('preserved')")
        .unwrap();
    drop(semantic);
    let (plugin, _) = runtime(root.path(), json!({}));
    command(&plugin, root.path(), "list", json!({}))
        .await
        .unwrap();
    assert_eq!(semantic_rows(root.path()).len(), 0);
    let semantic = rusqlite::Connection::open(root.path().join("kb/semantic_index.db")).unwrap();
    assert_eq!(
        semantic
            .query_row("SELECT value FROM keep", [], |row| row.get::<_, String>(0))
            .unwrap(),
        "preserved"
    );
    drop(semantic);
    std::fs::write(&path, b"not sqlite").unwrap();
    assert!(command(&plugin, root.path(), "list", json!({}))
        .await
        .is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"not sqlite");
}
