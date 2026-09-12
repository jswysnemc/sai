use super::knowledge_support::*;
use serde_json::{json, Value};

/// 【知识库命令测试】【全部入口】原管理语法映射同一组 Lua 命令，分页整数保留精度
/// @returns 无；目录、搜索、分页、删除及禁用嵌入输出一致
#[tokio::test]
async fn knowledge_commands_preserve_listing_search_read_remove_and_stats() {
    let root = tempfile::tempdir().unwrap();
    seed(
        root.path(),
        &json!({"note.md":"one\ntwo\nthree\n","second.md":"another note\n"}),
        true,
    );
    let (plugin, _) = runtime(root.path(), json!({"language":"en"}));
    assert_eq!(
        command(&plugin, root.path(), "list", json!({}))
            .await
            .unwrap(),
        "note.md\t14 bytes\nsecond.md\t13 bytes"
    );
    let files: Value = serde_json::from_str(
        &command(&plugin, root.path(), "list", json!({"format":"json"}))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(files.as_array().unwrap().len(), 2);
    assert_eq!(
        command(
            &plugin,
            root.path(),
            "read",
            json!({"file":"note.md","start":"2","lines":"1"})
        )
        .await
        .unwrap(),
        "=== note.md | lines 2-2 / 3 ===\ntwo\n\n... 1 more lines; continue with start_line=3"
    );
    assert!(command(
        &plugin,
        root.path(),
        "read",
        json!({"file":"note.md","start":"18446744073709551615"})
    )
    .await
    .unwrap()
    .contains("start_line 18446744073709551615 out of range"));
    let found: Value = serde_json::from_str(
        &command(
            &plugin,
            root.path(),
            "find",
            json!({"query":".md","limit":"0"}),
        )
        .await
        .unwrap(),
    )
    .unwrap();
    assert_eq!(found["total_matches"], 1);
    let found: Value = serde_json::from_str(
        &command(&plugin, root.path(), "search", json!({"query":"two"}))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(found["results"][0]["path"], "note.md");
    assert_eq!(
        command(&plugin, root.path(), "reindex", json!({}))
            .await
            .unwrap(),
        "keyword index is rebuilt on demand; files tracked: 2"
    );
    assert_eq!(
        command(&plugin, root.path(), "embed-reindex", json!({}))
            .await
            .unwrap(),
        "embedding is disabled"
    );
    assert_eq!(
        command(&plugin, root.path(), "embed-reindex", json!({"quiet":true}))
            .await
            .unwrap(),
        ""
    );
    assert_eq!(
        command(&plugin, root.path(), "remove", json!({"file":"note.md"}))
            .await
            .unwrap(),
        "removed note.md"
    );
    let stats: Value = serde_json::from_str(
        &command(&plugin, root.path(), "stats", json!({}))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(stats["files"], 1);
    assert_eq!(stats["semantic_chunks"], 0);
}

/// 【知识库命令测试】【目录导入】导入目录保留目录名和层级，跳过不支持、空白和非法编码的文件
/// @returns 无；成功集合及实际正文相同
#[tokio::test]
async fn knowledge_directory_import_preserves_names_and_skips_invalid_files() {
    let root = tempfile::tempdir().unwrap();
    let input = root.path().join("input/folder");
    std::fs::create_dir_all(input.join("nested")).unwrap();
    for (name, bytes) in [
        ("one.md", b"one".as_slice()),
        ("nested/two.txt", b"two"),
        ("image.png", b"unsupported"),
        ("empty.txt", b""),
        ("invalid.txt", b"\xff"),
    ] {
        std::fs::write(input.join(name), bytes).unwrap();
    }
    let (plugin, _) = runtime(root.path(), json!({}));
    let mut files_added: Vec<String> = serde_json::from_str(
        &command(
            &plugin,
            root.path(),
            "add",
            json!({"path":input,"format":"json"}),
        )
        .await
        .unwrap(),
    )
    .unwrap();
    files_added.sort();
    assert_eq!(files_added, vec!["folder/nested/two.txt", "folder/one.md"]);
    assert_eq!(
        files(root.path()),
        json!({"folder/one.md":"one","folder/nested/two.txt":"two"})
    );
}

/// 【知识库命令测试】【参数拒绝】错误 JSON、字段类型与超范围整数在任何文件变更前失败
/// @returns 无；拒绝不能创建知识库目录
#[tokio::test]
async fn knowledge_commands_validate_types_fields_and_full_unsigned_integers() {
    let root = tempfile::tempdir().unwrap();
    let (plugin, _) = runtime(root.path(), json!({}));
    for (name, text) in [
        ("list", "[]"),
        ("list", "null"),
        ("stats", "{\"unknown\":true}"),
        ("list", "{\"format\":7}"),
        ("search", "{\"query\":[]}"),
        ("find", "{\"query\":true}"),
        ("read", "{\"file\":\"x.md\",\"start\":-1}"),
        ("read", "{\"file\":\"x.md\",\"start\":1.5}"),
        (
            "read",
            "{\"file\":\"x.md\",\"start\":\"18446744073709551616\"}",
        ),
        ("add", "{\"path\":\"source\",\"name\":{}}"),
        ("embed-reindex", "{\"quiet\":\"true\"}"),
    ] {
        assert!(
            plugin
                .call_command(name, text, context(root.path(), true))
                .await
                .is_err(),
            "{name}: {text}"
        );
        assert!(!root.path().join("kb").exists(), "{name}: {text}");
    }
}

/// 【知识库命令测试】【导入失败】文件发布失败必须返回错误，不能报告跳过后留下一份隐藏恢复记录
/// @returns 无；下一次管理调用可以恢复前一次导入
#[tokio::test]
async fn knowledge_directory_import_reports_failed_commits_instead_of_silent_skips() {
    let root = tempfile::tempdir().unwrap();
    seed(root.path(), &json!({}), true);
    let input = root.path().join("input/folder");
    std::fs::create_dir_all(&input).unwrap();
    std::fs::write(input.join("one.md"), "one").unwrap();
    let (plugin, host) = runtime(root.path(), json!({}));
    *host.fail_publish.lock().unwrap() = Some("kb_meta.db".into());
    assert!(command(&plugin, root.path(), "add", json!({"path":input}))
        .await
        .is_err());
    assert!(root.path().join("kb/pending-write.json").exists());
    *host.fail_publish.lock().unwrap() = None;
    command(&plugin, root.path(), "reindex", json!({}))
        .await
        .unwrap();
    assert_eq!(files(root.path()), json!({"folder/one.md":"one"}));
}

/// 【知识库命令测试】【隐藏文件标题】空标题使用文件主名，前导点名称不丢失
/// @returns 无；默认标题保留原文件名称语义
#[tokio::test]
async fn knowledge_upload_empty_title_preserves_dotfile_stems() {
    let root = tempfile::tempdir().unwrap();
    let (plugin, _) = runtime(root.path(), json!({}));
    for (name, heading) in [
        (".env", ".env"),
        (".bashrc", ".bashrc"),
        (".notes.md", ".notes"),
        ("normal.md", "normal"),
    ] {
        call(
            &plugin,
            root.path(),
            "upload_text_to_knowledge_base",
            json!({"file_name":name,"title":"","content":"material"}),
            true,
        )
        .await
        .unwrap();
        let body = std::fs::read_to_string(root.path().join("kb/files").join(name)).unwrap();
        assert!(body.starts_with(&format!("# {heading}\n")));
    }
}
