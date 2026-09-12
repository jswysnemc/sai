use super::{knowledge_host::KnowledgeHost, knowledge_support::*};
use sai_plugin_runtime::host::HttpResponse;
use serde_json::{json, Value};

/// 【知识库边界测试】【默认最大正文】默认 1 MiB 文件可完整导入和读取，超限文件不覆盖旧正文
/// @returns 无；存储字节、元数据和读出内容保持完整
#[tokio::test]
async fn knowledge_default_maximum_file_fits_declared_budgets_without_truncation() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("input")).unwrap();
    let path = root.path().join("input/large.md");
    let content = "x".repeat(1024 * 1024);
    std::fs::write(&path, &content).unwrap();
    let (plugin, _) = runtime(root.path(), json!({}));
    command(&plugin, root.path(), "add", json!({"path":path}))
        .await
        .unwrap();
    let output = call(
        &plugin,
        root.path(),
        "read_knowledge_base_file",
        json!({"file_name":"large.md","max_lines":1}),
        false,
    )
    .await
    .unwrap();
    assert!(output.as_str().unwrap().ends_with(&content));
    std::fs::write(&path, format!("{content}x")).unwrap();
    assert!(command(&plugin, root.path(), "add", json!({"path":path}))
        .await
        .is_err());
    assert_eq!(
        std::fs::read(root.path().join("kb/files/large.md")).unwrap(),
        content.as_bytes()
    );
}

/// 【知识库边界测试】【多批语义发布】超过单批大小时只发布最终完整快照，提交失败保留全部旧块
/// @returns 无；同一文件 40 个块可以完整重建
#[tokio::test]
async fn knowledge_embedding_batches_publish_atomically_and_preserve_old_snapshot_on_failure() {
    let root = tempfile::tempdir().unwrap();
    seed(root.path(), &json!({"note.md":"a".repeat(128*40)}), true);
    semantic_row(root.path(), "note.md", "old", "[0,1]");
    let host = KnowledgeHost::new(root.path());
    let plugin = configured(
        root.path(),
        &embedding_config(),
        json!({"semantic_chunk_chars":128,"semantic_chunk_overlap":0}),
        host.clone(),
        "",
        |_| {},
    );
    for _ in 0..80 {
        host.responses.lock().unwrap().push_back(HttpResponse {
            status: 200,
            headers: Default::default(),
            text: r#"{"data":[{"embedding":[0.1,0.2]}]}"#.into(),
        });
    }
    let original = std::fs::read(root.path().join("kb/semantic_index.db")).unwrap();
    *host.fail_publish.lock().unwrap() = Some("semantic_index.db".into());
    assert!(
        command(&plugin, root.path(), "embed-reindex", json!({"quiet":true}))
            .await
            .is_err()
    );
    assert_eq!(
        std::fs::read(root.path().join("kb/semantic_index.db")).unwrap(),
        original
    );
    *host.fail_publish.lock().unwrap() = None;
    assert_eq!(
        command(&plugin, root.path(), "embed-reindex", json!({}))
            .await
            .unwrap(),
        "indexed semantic chunks: 40"
    );
    assert_eq!(semantic_rows(root.path()).len(), 40);
}

/// 【知识库边界测试】【WAL 与镜像上限】不把 WAL 或超大镜像当成空库，任何拒绝都保留原字节
/// @returns 无；只读和管理查询均不能发布截断索引
#[tokio::test]
async fn knowledge_rejects_wal_and_oversized_indexes_without_overwriting_them() {
    let root = tempfile::tempdir().unwrap();
    seed(root.path(), &json!({"note.md":"keep"}), true);
    let path = root.path().join("kb/kb_meta.db");
    let original = std::fs::read(&path).unwrap();
    let mut wal = original.clone();
    wal[18] = 2;
    wal[19] = 2;
    for bytes in [wal, vec![b'x'; 8 * 1024 * 1024 + 1]] {
        std::fs::write(&path, &bytes).unwrap();
        let (plugin, _) = runtime(root.path(), json!({}));
        assert!(call(
            &plugin,
            root.path(),
            "search_knowledge_base_by_name",
            json!({"file_name_query":"note"}),
            false
        )
        .await
        .is_err());
        assert!(command(&plugin, root.path(), "list", json!({}))
            .await
            .is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(files(root.path()), json!({"note.md":"keep"}));
    }
}

/// 【知识库边界测试】【目录截断】枚举超过 1024 项时整体拒绝，不能把未完整扫描的目录报为成功
/// @returns 无；目标正文尚未写入
#[tokio::test]
async fn knowledge_import_rejects_truncated_directory_listings() {
    let root = tempfile::tempdir().unwrap();
    let input = root.path().join("input");
    std::fs::create_dir(&input).unwrap();
    for index in 0..1025 {
        std::fs::write(input.join(format!("{index}.md")), "one").unwrap();
    }
    let (plugin, _) = runtime(root.path(), json!({}));
    let error = command(&plugin, root.path(), "add", json!({"path":input}))
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("directory exceeds entry limit"));
    assert_eq!(files(root.path()), Value::Object(Default::default()));
}

/// 【知识库边界测试】【常见向量规模】64 个 1536 维向量必须在默认预算内完成检索
/// @returns 无；完整扫描原索引后返回原定 top-k，不因重复复制镜像退回空结果
#[tokio::test]
async fn knowledge_search_handles_realistic_embedding_dimensions_and_index_pages() {
    let root = tempfile::tempdir().unwrap();
    seed(root.path(), &json!({"note.md":"local material"}), true);
    let vector: Vec<f32> = (0..1536)
        .map(|index| (index % 7) as f32 * 0.01234567 - 0.04)
        .collect();
    let encoded = serde_json::to_string(&vector).unwrap();
    for index in 0..64 {
        semantic_row(
            root.path(),
            &format!("doc-{index}.md"),
            "stored material",
            &encoded,
        );
    }
    let host = KnowledgeHost::new(root.path());
    host.responses.lock().unwrap().push_back(HttpResponse {
        status: 200,
        headers: Default::default(),
        text: json!({"data":[{"embedding":vector}]}).to_string(),
    });
    let plugin = configured(
        root.path(),
        &embedding_config(),
        json!({}),
        host,
        "",
        |_| {},
    );
    let result = call(
        &plugin,
        root.path(),
        "search_knowledge_base",
        json!({"query":"unmatched"}),
        false,
    )
    .await
    .unwrap();
    assert_eq!(result["semantic_used"], true);
    assert_eq!(result["total_matches"], 5);
    assert_eq!(result["results"][0]["score"], 200.0);
}

/// 【知识库边界测试】【宽字段分页】合法向量包含空白时自动缩小查询页，完整消费旧索引
/// @returns 无；33 行跨页结果不遗漏、不重复，原索引字节保持不变
#[tokio::test]
async fn knowledge_search_reduces_oversized_pages_without_losing_or_repeating_rows() {
    let root = tempfile::tempdir().unwrap();
    seed(root.path(), &json!({"note.md":"local material"}), true);
    let encoded = format!("{}[1,0]", " ".repeat(32768));
    for index in 0..33 {
        semantic_row(
            root.path(),
            &format!("doc-{index}.md"),
            "stored material",
            &encoded,
        );
    }
    let index_path = root.path().join("kb/semantic_index.db");
    let original = std::fs::read(&index_path).unwrap();
    let host = KnowledgeHost::new(root.path());
    host.responses.lock().unwrap().push_back(HttpResponse {
        status: 200,
        headers: Default::default(),
        text: json!({"data":[{"embedding":[1,0]}]}).to_string(),
    });
    let plugin = configured(
        root.path(),
        &embedding_config(),
        json!({"semantic_top_k":40}),
        host,
        "",
        |_| {},
    );
    let result = call(
        &plugin,
        root.path(),
        "search_knowledge_base",
        json!({"query":"unmatched","max_results":40}),
        false,
    )
    .await
    .unwrap();
    assert_eq!(result["semantic_used"], true);
    assert_eq!(result["total_matches"], 33);
    let names = result["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["path"].as_str().unwrap())
        .collect::<Vec<_>>();
    let expected = (0..33)
        .map(|index| format!("doc-{index}.md"))
        .collect::<Vec<_>>();
    assert_eq!(names, expected);
    assert_eq!(std::fs::read(index_path).unwrap(), original);
}
