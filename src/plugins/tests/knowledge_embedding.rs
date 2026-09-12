use super::{knowledge_host::KnowledgeHost, knowledge_support::*};
use sai_plugin_runtime::{
    host::{HttpResponse, ScheduledStatus},
    PluginRuntime,
};
use serde_json::{json, Value};
use std::{
    path::Path,
    sync::{atomic::Ordering, Arc, Mutex},
    time::Duration,
};

/// 【知识库嵌入测试】【响应队列】输入宿主、状态码和正文；返回无，追加固定 HTTP 响应
fn respond(host: &KnowledgeHost, status: u16, text: impl Into<String>) {
    host.responses.lock().unwrap().push_back(HttpResponse {
        status,
        text: text.into(),
        headers: Default::default(),
    });
}

/// 【知识库嵌入测试】【完整实例】输入隔离目录；返回启用嵌入的实际包及受控宿主
fn fixture(root: &Path) -> (PluginRuntime, Arc<KnowledgeHost>) {
    let host = KnowledgeHost::new(root);
    let plugin = configured(
        root,
        &embedding_config(),
        json!({}),
        host.clone(),
        "",
        |_| {},
    );
    (plugin, host)
}

/// 【知识库嵌入测试】【旧向量与只读 POST】使用旧供应商块并保留原向量选择，不初始化或重写索引
/// @returns 无；端点、模型、鉴权和最终评分匹配
#[tokio::test]
async fn knowledge_semantic_search_reuses_legacy_vectors_and_readonly_post() {
    let root = tempfile::tempdir().unwrap();
    seed(root.path(), &json!({"note.md":"local material"}), true);
    semantic_row(root.path(), "note.md", " legacy\n snippet ", "[1,0]");
    semantic_row(root.path(), "other.md", "invalid", "not json");
    let original = std::fs::read(root.path().join("kb/semantic_index.db")).unwrap();
    let (plugin, host) = fixture(root.path());
    respond(&host, 200, r#"{"data":[{"embedding":[1,0]}]}"#);
    let output = call(
        &plugin,
        root.path(),
        "search_knowledge_base",
        json!({"query":"unmatched"}),
        false,
    )
    .await
    .unwrap();
    assert_eq!(output["semantic_used"], true);
    assert_eq!(output["results"][0]["path"], "note.md");
    assert_eq!(output["results"][0]["score"], 200.0);
    assert_eq!(output["results"][0]["snippets"], json!(["legacy snippet"]));
    let requests = host.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].url, "https://embedding.test/v1/embeddings");
    assert_eq!(requests[0].method, "POST");
    assert_eq!(
        requests[0].headers["authorization"],
        "Bearer fixture-embedding-key"
    );
    assert_eq!(
        serde_json::from_str::<Value>(requests[0].body.as_ref().unwrap()).unwrap(),
        json!({"model":"fixture-vector","input":"unmatched"})
    );
    assert_eq!(
        std::fs::read(root.path().join("kb/semantic_index.db")).unwrap(),
        original
    );
}

/// 【知识库嵌入测试】【关键词和错误回退】强关键词跳过网络，无效响应退回完整关键词结果
/// @returns 无；HTTP 错误不会伪装成成功语义匹配
#[tokio::test]
async fn knowledge_strong_keywords_skip_http_and_bad_embeddings_fall_back() {
    let root = tempfile::tempdir().unwrap();
    seed(root.path(), &json!({"rust.md":"rust language"}), true);
    let (plugin, host) = fixture(root.path());
    let strong = call(
        &plugin,
        root.path(),
        "search_knowledge_base",
        json!({"query":"rust"}),
        false,
    )
    .await
    .unwrap();
    assert_eq!(strong["semantic_used"], false);
    assert_eq!(strong["total_matches"], 1);
    assert!(host.requests.lock().unwrap().is_empty());
    for (status, body) in [
        (500, "fixture failure"),
        (200, "{broken"),
        (200, "{\"data\":[]}"),
        (200, "{\"data\":[{\"embedding\":[1e100]}]}"),
    ] {
        respond(&host, status, body);
        let output = call(
            &plugin,
            root.path(),
            "search_knowledge_base",
            json!({"query":"unmatched"}),
            false,
        )
        .await
        .unwrap();
        assert_eq!(output["semantic_used"], false);
        assert_eq!(output["total_matches"], 0);
    }
}

/// 【知识库嵌入测试】【单文件重建】成功块按原字节偏移保存，其他文件的旧块不受替换影响
/// @returns 无；摘要、模型及单精度向量可以从原 SQLite 表读取
#[tokio::test]
async fn knowledge_reindex_replaces_file_chunks_and_preserves_other_rows() {
    let root = tempfile::tempdir().unwrap();
    seed(root.path(), &json!({"note.md":"中文正文"}), true);
    semantic_row(root.path(), "note.md", "old", "[0,1]");
    semantic_row(root.path(), "other.md", "keep", "[0,1]");
    let (plugin, host) = fixture(root.path());
    respond(
        &host,
        200,
        r#"{"data":[{"embedding":[0.1,-0.0,1.23456789]}]}"#,
    );
    assert_eq!(
        command(&plugin, root.path(), "embed-reindex", json!({}))
            .await
            .unwrap(),
        "indexed semantic chunks: 1"
    );
    let rows = semantic_rows(root.path());
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].0, "other.md");
    assert_eq!(rows[1].1, "中文正文");
    let vector: Vec<f32> = serde_json::from_str(&rows[1].2).unwrap();
    assert_eq!(vector, vec![0.1, -0.0, 1.23456789]);
    assert!(vector[1].is_sign_negative());
    assert_eq!(
        rows[1].3,
        super::binary_conditional_support::digest("中文正文".as_bytes())
    );
    let db = rusqlite::Connection::open(root.path().join("kb/semantic_index.db")).unwrap();
    let record: (String,String,i64,i64)=db.query_row("SELECT provider_id,model,start_char,end_char FROM semantic_chunks WHERE file_name='note.md'",[],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).unwrap();
    assert_eq!(
        record,
        ("embedding-test".into(), "fixture-vector".into(), 0, 12)
    );
}

/// 【知识库嵌入测试】【编辑和删除竞争】网络等待不持有数据锁，晚到向量不能覆盖已修改文件
/// @returns 无；并发变更可完成，旧重建最终不发布任何过期块
#[tokio::test]
async fn knowledge_reindex_cannot_publish_vectors_after_concurrent_edit_or_removal() {
    for remove in [false, true] {
        let root = tempfile::tempdir().unwrap();
        seed(root.path(), &json!({"note.md":"old body\n"}), true);
        semantic_row(root.path(), "note.md", "old vector", "[0,1]");
        let (plugin, host) = fixture(root.path());
        host.pause_http.store(true, Ordering::SeqCst);
        respond(&host, 200, r#"{"data":[{"embedding":[1,0]}]}"#);
        let directory = root.path().to_path_buf();
        let job =
            tokio::spawn(
                async move { command(&plugin, &directory, "embed-reindex", json!({})).await },
            );
        tokio::time::timeout(Duration::from_secs(3), host.http_started.notified())
            .await
            .unwrap();
        let second = configured(
            root.path(),
            &embedding_config(),
            json!({}),
            host.clone(),
            "",
            |_| {},
        );
        let (name, args) = if remove {
            ("remove_knowledge_base_file", json!({"file_name":"note.md"}))
        } else {
            (
                "edit_knowledge_base_file",
                json!({"file_name":"note.md","start_line":1,"end_line":1,"replacement":"new body"}),
            )
        };
        tokio::time::timeout(
            Duration::from_secs(3),
            call(&second, root.path(), name, args, true),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(semantic_rows(root.path()).is_empty());
        host.http_release.notify_one();
        assert_eq!(job.await.unwrap().unwrap(), "indexed semantic chunks: 0");
        assert!(semantic_rows(root.path()).is_empty());
    }
}

/// 【知识库嵌入测试】【任务合并】连续写入合并等待任务，已有运行任务之后保留一次重新扫描
/// @returns 无；持久参数不包含正文、目录或凭据
#[tokio::test]
async fn knowledge_background_jobs_coalesce_scheduled_work_but_follow_running_work() {
    let root = tempfile::tempdir().unwrap();
    let (plugin, host) = fixture(root.path());
    for index in 0..3 {
        call(
            &plugin,
            root.path(),
            "upload_text_to_knowledge_base",
            json!({"file_name":format!("{index}.md"),"content":"material"}),
            true,
        )
        .await
        .unwrap();
    }
    assert_eq!(host.jobs.tasks.lock().unwrap().len(), 1);
    host.jobs.tasks.lock().unwrap()[0].status = ScheduledStatus::Running;
    let args: Value = serde_json::from_str(&host.jobs.tasks.lock().unwrap()[0].arguments).unwrap();
    let worker = configured(
        root.path(),
        &embedding_config(),
        json!({}),
        host.clone(),
        "",
        |_| {},
    );
    host.pause_http.store(true, Ordering::SeqCst);
    for _ in 0..3 {
        respond(&host, 200, r#"{"data":[{"embedding":[1,0]}]}"#);
    }
    let directory = root.path().to_path_buf();
    let running =
        tokio::spawn(async move { command(&worker, &directory, "embed-reindex", args).await });
    tokio::time::timeout(Duration::from_secs(3), host.http_started.notified())
        .await
        .unwrap();
    call(
        &plugin,
        root.path(),
        "edit_knowledge_base_file",
        json!({"file_name":"0.md","start_line":1,"end_line":1,"replacement":"new title"}),
        true,
    )
    .await
    .unwrap();
    let tasks = host.jobs.tasks.lock().unwrap();
    assert_eq!(tasks.len(), 2);
    for task in tasks.iter() {
        assert_eq!(task.command, "embed-reindex");
        let args: Value = serde_json::from_str(&task.arguments).unwrap();
        assert_eq!(args["quiet"], true);
        assert_eq!(args["background"], true);
        assert!(args["ticket"].as_str().unwrap().parse::<u64>().is_ok());
        assert_eq!(args.as_object().unwrap().len(), 3);
    }
    drop(tasks);
    host.pause_http.store(false, Ordering::SeqCst);
    host.http_release.notify_one();
    running.await.unwrap().unwrap();
}

/// 【知识库嵌入测试】【错误脱敏】进度只包含有界 UTF-8 文字，服务回显凭据必须替换
/// @returns 无；失败块没有进入新索引，进度不含密钥
#[tokio::test]
async fn knowledge_embedding_errors_redact_credentials_and_keep_valid_utf8() {
    let root = tempfile::tempdir().unwrap();
    seed(root.path(), &json!({"note.md":"body"}), true);
    let (plugin, host) = fixture(root.path());
    respond(
        &host,
        401,
        format!("fixture-embedding-key {}", "错误".repeat(2000)),
    );
    let messages = Arc::new(Mutex::new(Vec::new()));
    let capture = messages.clone();
    let mut ctx = context(root.path(), true);
    ctx.progress = Some(Arc::new(move |message| {
        capture.lock().unwrap().push(message)
    }));
    assert_eq!(
        plugin
            .call_command("embed-reindex", "{}", ctx)
            .await
            .unwrap(),
        "indexed semantic chunks: 0"
    );
    let messages = messages.lock().unwrap();
    assert_eq!(messages.len(), 1);
    assert!(!messages[0].contains("fixture-embedding-key"));
    assert!(messages[0].contains("[redacted]"));
    assert!(messages[0].len() <= 4000);
}

/// 【知识库嵌入测试】【取消与竞争提示】运行中的重建使手动重复请求立即返回，取消后旧索引仍可读取
/// @returns 无；新锁不提示删除旧锁文件，真实旧锁继续阻止兼容进程竞争
#[tokio::test]
async fn knowledge_embedding_cancellation_preserves_old_index_and_releases_busy_lock() {
    let root = tempfile::tempdir().unwrap();
    seed(root.path(), &json!({"note.md":"body"}), true);
    semantic_row(root.path(), "note.md", "old", "[0,1]");
    let original = std::fs::read(root.path().join("kb/semantic_index.db")).unwrap();
    let (plugin, host) = fixture(root.path());
    host.pause_http.store(true, Ordering::SeqCst);
    let directory = root.path().to_path_buf();
    let task =
        tokio::spawn(async move { command(&plugin, &directory, "embed-reindex", json!({})).await });
    tokio::time::timeout(Duration::from_secs(3), host.http_started.notified())
        .await
        .unwrap();
    let (second, other) = fixture(root.path());
    assert_eq!(
        command(&second, root.path(), "embed-reindex", json!({}))
            .await
            .unwrap(),
        "embedding reindex already running"
    );
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(
        std::fs::read(root.path().join("kb/semantic_index.db")).unwrap(),
        original
    );
    respond(&other, 200, r#"{"data":[{"embedding":[1,0]}]}"#);
    command(&second, root.path(), "embed-reindex", json!({}))
        .await
        .unwrap();
    std::fs::write(root.path().join("kb/embedding.lock"), b"").unwrap();
    let busy = command(&second, root.path(), "embed-reindex", json!({}))
        .await
        .unwrap();
    assert!(busy.contains("lock file:"));
    assert!(busy.contains("remove the stale lock file"));
}

/// 【知识库嵌入测试】【等待者合并】宿主标记 Running 但还未取得嵌入锁的任务继续合并后续修改
/// @returns 无；等待者只有一项，避免快速编辑耗尽活动任务配额
#[tokio::test]
async fn knowledge_background_jobs_coalesce_workers_still_waiting_for_the_embedding_lock() {
    let root = tempfile::tempdir().unwrap();
    let (plugin, host) = fixture(root.path());
    call(
        &plugin,
        root.path(),
        "upload_text_to_knowledge_base",
        json!({"file_name":"note.md","content":"material"}),
        true,
    )
    .await
    .unwrap();
    host.jobs.tasks.lock().unwrap()[0].status = ScheduledStatus::Running;
    for index in 0..3 {
        call(&plugin,root.path(),"edit_knowledge_base_file",json!({"file_name":"note.md","start_line":1,"end_line":1,"replacement":format!("title {index}")}),true).await.unwrap();
    }
    assert_eq!(host.jobs.tasks.lock().unwrap().len(), 1);
}

/// 【知识库嵌入测试】【相对目录隔离】不同工作目录中的同名相对库不能合并成一个后台任务
/// @returns 无；共享应用状态时仍分别重建两个真实目录
#[tokio::test]
async fn knowledge_background_jobs_distinguish_relative_roots_across_workdirs() {
    let root = tempfile::tempdir().unwrap();
    let host = KnowledgeHost::new(root.path());
    let plugin = configured(
        root.path(),
        &embedding_config(),
        json!({"data_dir":"kb"}),
        host.clone(),
        "",
        |_| {},
    );
    for name in ["first", "second"] {
        let work = root.path().join(name);
        std::fs::create_dir(&work).unwrap();
        call(
            &plugin,
            &work,
            "upload_text_to_knowledge_base",
            json!({"file_name":"note.md","content":"material"}),
            true,
        )
        .await
        .unwrap();
    }
    assert_eq!(host.jobs.tasks.lock().unwrap().len(), 2);
}
