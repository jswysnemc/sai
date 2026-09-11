use super::binary_conditional_support::digest;
use super::sqlite_support::*;
use crate::{config::AppConfig, paths::SaiPaths, tools::knowledge_base::KnowledgeBase};
use serde_json::json;
use std::sync::Arc;

/// 【数据库文件测试】【旧索引接续】正式知识库创建的两种表均可经 Lua 快照接口读取
/// @returns 无；查询不修改元数据、向量索引或原文件目录
#[tokio::test]
async fn sqlite_queries_read_real_knowledge_indexes_without_touching_disk_bytes() {
    let root = tempfile::tempdir().unwrap();
    let mut config = AppConfig::default();
    config.plugins.knowledge_base.data_dir = root.path().join("allowed").display().to_string();
    KnowledgeBase::new(config, SaiPaths::for_tests(root.path()))
        .unwrap()
        .init()
        .unwrap();
    let meta = root.path().join("allowed/kb_meta.db");
    let semantic = root.path().join("allowed/semantic_index.db");
    {
        let connection = rusqlite::Connection::open(&meta).unwrap();
        connection
            .execute(
                "INSERT INTO files VALUES(?1,?2,?3,?4,?5,?6)",
                rusqlite::params!["测试.md", "files/测试.md", 18, 10.5, "digest", 20.25],
            )
            .unwrap();
        let connection = rusqlite::Connection::open(&semantic).unwrap();
        connection.execute_batch("INSERT INTO semantic_chunks(provider_id,model,file_name,content_sha256,chunk_index,start_char,end_char,text,embedding_json,created_at) VALUES('provider','embedding','测试.md','digest',0,0,2,'内容','[0.25,-0.5]',20.25);").unwrap();
    }
    let before_meta = std::fs::read(&meta).unwrap();
    let before_semantic = std::fs::read(&semantic).unwrap();
    let plugin = runtime(root.path(), |grants| grants.binary.write_paths.clear());
    let output=call(&plugin,root.path(),"query",json!({"path":"allowed/kb_meta.db","query":{"table":"files","columns":["name","path","size_bytes","mtime","content_sha256","updated_at"]}}),false).await.unwrap();
    assert_eq!(
        output["data"]["rows"],
        json!([{"name":"测试.md","path":"files/测试.md","size_bytes":18,"mtime":10.5,"content_sha256":"digest","updated_at":20.25}])
    );
    let output=call(&plugin,root.path(),"query",json!({"path":"allowed/semantic_index.db","query":{"table":"semantic_chunks","columns":["id","file_name","text","embedding_json","chunk_index","created_at"]}}),false).await.unwrap();
    assert_eq!(
        output["data"]["rows"],
        json!([{"id":1,"file_name":"测试.md","text":"内容","embedding_json":"[0.25,-0.5]","chunk_index":0,"created_at":20.25}])
    );
    assert_eq!(std::fs::read(meta).unwrap(), before_meta);
    assert_eq!(std::fs::read(semantic).unwrap(), before_semantic);
    assert_eq!(
        std::fs::read_dir(root.path().join("allowed"))
            .unwrap()
            .count(),
        3
    );
}

/// 【数据库文件测试】【创建与更新】缺失条件只发布一次，新数据库可由原生 SQLite 读取
/// @returns 无；计算副本和发布副本都不修改输入文件
#[tokio::test]
async fn snapshot_publication_is_conditional_and_preserves_the_source_file() {
    let root = tempfile::tempdir().unwrap();
    let before = seed(root.path());
    let plugin = runtime(root.path(), |_| {});
    let args = json!({"source":"allowed/notes.db","path":"allowed/copy.db","changes":changes("published")});
    assert_eq!(
        call(&plugin, root.path(), "apply", args.clone(), true)
            .await
            .unwrap(),
        true
    );
    assert_eq!(
        call(&plugin, root.path(), "apply", args, true)
            .await
            .unwrap(),
        false
    );
    assert_eq!(
        std::fs::read(root.path().join("allowed/notes.db")).unwrap(),
        before
    );
    let output = call(
        &plugin,
        root.path(),
        "query",
        query("allowed/copy.db"),
        false,
    )
    .await
    .unwrap();
    assert_eq!(output["data"]["rows"], json!([{"id":1,"text":"published"}]));
    let connection = rusqlite::Connection::open(root.path().join("allowed/copy.db")).unwrap();
    assert_eq!(
        connection
            .query_row("PRAGMA quick_check", [], |row| row.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
    assert_eq!(
        connection
            .query_row("SELECT text FROM notes", [], |row| row.get::<_, String>(0))
            .unwrap(),
        "published"
    );
}

/// 【数据库文件测试】【多实例竞争】同一旧修订只能由一个真实宿主发布完整数据库
/// @returns 无；磁盘记录恰好来自成功者，其他请求返回条件冲突
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn competing_sqlite_snapshots_cannot_overwrite_the_same_revision_twice() {
    let root = tempfile::tempdir().unwrap();
    let before = seed(root.path());
    let barrier = Arc::new(tokio::sync::Barrier::new(8));
    let mut calls = tokio::task::JoinSet::new();
    for index in 0..8 {
        let plugin = runtime(root.path(), |_| {});
        let path = root.path().to_path_buf();
        let barrier = barrier.clone();
        let args = json!({"source":"allowed/notes.db","path":"allowed/notes.db","expected":digest(&before),"changes":changes(&format!("revision-{index}"))});
        calls.spawn(async move {
            barrier.wait().await;
            (index, call(&plugin, &path, "apply", args, true).await)
        });
    }
    let mut winners = Vec::new();
    while let Some(result) = calls.join_next().await {
        let (index, result) = result.unwrap();
        if result.unwrap() == true {
            winners.push(index);
        }
    }
    assert_eq!(winners.len(), 1);
    let plugin = runtime(root.path(), |_| {});
    let output = call(
        &plugin,
        root.path(),
        "query",
        query("allowed/notes.db"),
        false,
    )
    .await
    .unwrap();
    assert_eq!(
        output["data"]["rows"],
        json!([{"id":1,"text":format!("revision-{}",winners[0])}])
    );
    assert_eq!(
        std::fs::read_dir(root.path().join("allowed"))
            .unwrap()
            .count(),
        1
    );
}

/// 【数据库文件测试】【失败原样保留】冲突、无效结构、固定容量不足及完整读取超限均不发布
/// @returns 无；整个原文件逐字节保持不变
#[tokio::test]
async fn failed_changes_and_reads_preserve_original_database_files() {
    let root = tempfile::tempdir().unwrap();
    let before = seed(root.path());
    let plugin = runtime(root.path(), |_| {});
    for extra in [
        json!({"changes":[{"op":"delete","table":"notes"},{"op":"insert","table":"notes","rows":[{"id":1,"text":"first"},{"id":1,"text":"duplicate"}]}]}),
        json!({"changes":[{"op":"sql","sql":"DROP TABLE notes"}]}),
        json!({"changes":changes(&"x".repeat(16384)),"capacity":8192}),
        json!({"changes":changes("unused"),"read_bytes":8191}),
    ] {
        let mut args = json!({"source":"allowed/notes.db","path":"allowed/notes.db","expected":digest(&before)});
        args.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        assert!(call(&plugin, root.path(), "apply", args, true)
            .await
            .is_err());
        assert_eq!(
            std::fs::read(root.path().join("allowed/notes.db")).unwrap(),
            before
        );
    }
    for damaged in [b"broken".to_vec(), vec![0; 1048577]] {
        std::fs::write(root.path().join("allowed/broken.db"), &damaged).unwrap();
        assert!(call(&plugin,root.path(),"apply",json!({"source":"allowed/broken.db","path":"allowed/broken.db","expected":digest(&damaged),"changes":changes("unused")}),true).await.is_err());
        assert_eq!(
            std::fs::read(root.path().join("allowed/broken.db")).unwrap(),
            damaged
        );
    }
}

/// 【数据库文件测试】【WAL 一致性】依赖 WAL 的活动数据库不能伪装成独立快照
/// @returns 无；拒绝时主文件与 WAL 不变，完成检查点并切换日志格式后可读
#[tokio::test]
async fn live_wal_databases_require_an_independent_rollback_journal_image() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("allowed")).unwrap();
    let path = root.path().join("allowed/notes.db");
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE notes(id INTEGER,text TEXT); INSERT INTO notes VALUES(1,'wal');").unwrap();
    let before = std::fs::read(&path).unwrap();
    let wal = std::fs::read(path.with_extension("db-wal")).unwrap();
    let plugin = runtime(root.path(), |_| {});
    let error = call(
        &plugin,
        root.path(),
        "query",
        query("allowed/notes.db"),
        false,
    )
    .await
    .unwrap_err();
    assert!(format!("{error:#}").contains("WAL"), "{error:#}");
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(std::fs::read(path.with_extension("db-wal")).unwrap(), wal);
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode=DELETE;")
        .unwrap();
    drop(connection);
    let output = call(
        &plugin,
        root.path(),
        "query",
        query("allowed/notes.db"),
        false,
    )
    .await
    .unwrap();
    assert_eq!(output["data"]["rows"], json!([{"id":1,"text":"wal"}]));
}

/// 【数据库文件测试】【路径隔离】授权内初始链接沿用文件契约，越界链接和相邻目录均拒绝
/// @returns 无；越界目标和链接本身不受修改
#[cfg(unix)]
#[tokio::test]
async fn sqlite_composition_preserves_file_grant_and_symlink_boundaries() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let before = seed(root.path());
    std::fs::write(outside.path().join("notes.db"), &before).unwrap();
    symlink("notes.db", root.path().join("allowed/alias.db")).unwrap();
    symlink(
        outside.path().join("notes.db"),
        root.path().join("allowed/escape.db"),
    )
    .unwrap();
    std::fs::create_dir(root.path().join("allowed-other")).unwrap();
    std::fs::write(root.path().join("allowed-other/notes.db"), &before).unwrap();
    let plugin = runtime(root.path(), |_| {});
    assert!(call(
        &plugin,
        root.path(),
        "query",
        query("allowed/alias.db"),
        false
    )
    .await
    .is_ok());
    for path in [
        "allowed/escape.db",
        "allowed-other/notes.db",
        "allowed/../allowed-other/notes.db",
    ] {
        assert!(call(&plugin, root.path(), "query", query(path), false)
            .await
            .is_err());
        assert!(call(&plugin,root.path(),"apply",json!({"source":"allowed/notes.db","path":path,"expected":digest(&before),"changes":changes("denied")}),true).await.is_err());
    }
    assert_eq!(
        std::fs::read(outside.path().join("notes.db")).unwrap(),
        before
    );
    assert_eq!(call(&plugin,root.path(),"apply",json!({"source":"allowed/alias.db","path":"allowed/alias.db","expected":digest(&before),"changes":changes("linked")}),true).await.unwrap(),true);
    assert!(
        std::fs::symlink_metadata(root.path().join("allowed/alias.db"))
            .unwrap()
            .is_symlink()
    );
}
