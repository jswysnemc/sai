use super::{knowledge_host::KnowledgeHost, knowledge_support::*};
use crate::{
    config::AppConfig,
    paths::SaiPaths,
    plugins::{
        self,
        commands::{run_bundled, BundledCommand},
        knowledge_view, GrantUpdate,
    },
};
use serde_json::json;

/// 【知识库权限测试】【只读初始化】三项只读工具不创建知识库；查询既有数据不需要写入或删除授权
/// @returns 无；读取权限独立且原数据库字节不变
#[tokio::test]
async fn knowledge_readonly_tools_do_not_initialize_or_require_write_grants() {
    let root = tempfile::tempdir().unwrap();
    let plugin = configured(
        root.path(),
        &AppConfig::default(),
        json!({}),
        KnowledgeHost::new(root.path()),
        "",
        |caps| {
            caps.binary.write_paths.clear();
            caps.system.remove_paths.clear();
        },
    );
    assert_eq!(
        call(
            &plugin,
            root.path(),
            "search_knowledge_base",
            json!({"query":"material"}),
            false
        )
        .await
        .unwrap()["total_matches"],
        0
    );
    assert_eq!(
        call(
            &plugin,
            root.path(),
            "search_knowledge_base_by_name",
            json!({"file_name_query":"note"}),
            false
        )
        .await
        .unwrap()["total_matches"],
        0
    );
    assert!(call(
        &plugin,
        root.path(),
        "read_knowledge_base_file",
        json!({"file_name":"note.md"}),
        false
    )
    .await
    .is_err());
    assert!(!root.path().join("kb").exists());
    seed(root.path(), &json!({"note.md":"material"}), true);
    let before = std::fs::read(root.path().join("kb/kb_meta.db")).unwrap();
    assert_eq!(
        call(
            &plugin,
            root.path(),
            "search_knowledge_base",
            json!({"query":"material"}),
            false
        )
        .await
        .unwrap()["total_matches"],
        1
    );
    assert!(call(
        &plugin,
        root.path(),
        "read_knowledge_base_file",
        json!({"file_name":"note.md"}),
        false
    )
    .await
    .unwrap()
    .as_str()
    .unwrap()
    .contains("material"));
    assert_eq!(
        std::fs::read(root.path().join("kb/kb_meta.db")).unwrap(),
        before
    );
}

/// 【知识库权限测试】【独立撤权】目录、存储、输出和删除各自需要授权，可信写入不能补足缺失能力
/// @returns 无；被撤销操作不会删除原正文
#[tokio::test]
async fn knowledge_tools_enforce_read_write_remove_and_lock_grants() {
    for revoked in ["read", "write", "remove", "lock"] {
        let root = tempfile::tempdir().unwrap();
        seed(root.path(), &json!({"note.md":"keep"}), true);
        let plugin = configured(
            root.path(),
            &AppConfig::default(),
            json!({}),
            KnowledgeHost::new(root.path()),
            "",
            |caps| match revoked {
                "read" => caps.system.read_paths.clear(),
                "write" => caps.binary.write_paths.clear(),
                "remove" => caps.system.remove_paths.clear(),
                _ => caps.system.plugin_storage = false,
            },
        );
        assert!(
            call(
                &plugin,
                root.path(),
                "remove_knowledge_base_file",
                json!({"file_name":"note.md"}),
                true
            )
            .await
            .is_err(),
            "{revoked}"
        );
        assert_eq!(files(root.path()), json!({"note.md":"keep"}));
    }
}

/// 【知识库权限测试】【网络撤权】HTTP 来源和只读 POST 端点独立授权，撤销后关键词仍可使用
/// @returns 无；未授权请求没有进入宿主
#[tokio::test]
async fn knowledge_embedding_requires_origin_and_readonly_endpoint_grants() {
    for endpoint in [false, true] {
        let root = tempfile::tempdir().unwrap();
        seed(root.path(), &json!({"note.md":"body"}), true);
        let host = KnowledgeHost::new(root.path());
        let plugin = configured(
            root.path(),
            &embedding_config(),
            json!({}),
            host.clone(),
            "",
            |caps| {
                caps.http_read_only_post.clear();
                if !endpoint {
                    caps.http.clear();
                }
            },
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
        assert_eq!(result["semantic_used"], false);
        assert!(host.requests.lock().unwrap().is_empty());
    }
}

/// 【知识库权限测试】【显式导入与界面】用户选择的单文件临时获得读取授权，配置界面使用同一 Lua 业务
/// @returns 无；没有保存临时授权，禁用同时封锁管理和界面入口
#[tokio::test]
async fn knowledge_management_and_tui_share_explicit_import_and_disable_rules() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.plugins.knowledge_base.data_dir = root.path().join("kb").display().to_string();
    let source = root.path().join("source.md");
    std::fs::write(&source, "user selected file").unwrap();
    assert_eq!(
        knowledge_view::add(&paths, &config, &source).await.unwrap(),
        1
    );
    let entries = knowledge_view::list(&paths, &config).await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "source.md");
    assert_eq!(
        knowledge_view::stats(&paths, &config).await.unwrap()["files"],
        1
    );
    assert!(!paths.config_dir.join("plugins.jsonc").exists());
    let found = plugins::discovery::find(&config, &paths, "knowledge-base").unwrap();
    assert_eq!(found.capabilities().system.read_paths.len(), 1);
    assert!(!found
        .capabilities()
        .system
        .read_paths
        .contains(source.to_str().unwrap()));
    plugins::set_enabled(&config, &paths, "knowledge-base", false, GrantUpdate::Keep).unwrap();
    assert!(knowledge_view::list(&paths, &config).await.is_err());
    assert!(knowledge_view::remove(&paths, &config, "source.md")
        .await
        .is_err());
    plugins::set_enabled(&config, &paths, "knowledge-base", true, GrantUpdate::Keep).unwrap();
    knowledge_view::remove(&paths, &config, "source.md")
        .await
        .unwrap();
    assert_eq!(
        knowledge_view::stats(&paths, &config).await.unwrap()["files"],
        0
    );
}

/// 【知识库权限测试】【计划模式】六个写入入口和显式管理查询均遵循可信只读上下文
/// @returns 无；拒绝前不会初始化知识库目录
#[tokio::test]
async fn knowledge_writes_and_compatibility_commands_reject_plan_mode() {
    let root = tempfile::tempdir().unwrap();
    let (plugin, _) = runtime(root.path(), json!({}));
    for (tool, args) in [
        ("upload_text_to_knowledge_base", json!({"content":"body"})),
        (
            "edit_knowledge_base_file",
            json!({"file_name":"note.md","start_line":1,"end_line":1,"replacement":"new"}),
        ),
        ("remove_knowledge_base_file", json!({"file_name":"note.md"})),
    ] {
        assert!(call(&plugin, root.path(), tool, args, false).await.is_err());
    }
    let mut config = AppConfig::default();
    config.plugins.knowledge_base.data_dir = root.path().join("kb").display().to_string();
    for command in ["list", "stats", "reindex", "embed-reindex"] {
        assert!(run_bundled(
            &config,
            &SaiPaths::for_tests(root.path()),
            BundledCommand {
                plugin: "knowledge-base",
                command,
                arguments: json!({}),
                input: None,
                allow_writes: false
            }
        )
        .await
        .is_err());
    }
    assert!(!root.path().join("kb").exists());
}

/// 【知识库权限测试】【坏路径与符号链接】工具参数不能越过库根，索引中的路径字段也不能扩展读取范围
/// @returns 无；外部正文和索引都不变
#[cfg(unix)]
#[tokio::test]
async fn knowledge_paths_and_legacy_index_cannot_read_or_change_external_files() {
    let root = tempfile::tempdir().unwrap();
    seed(root.path(), &json!({"note.md":"inside"}), true);
    let outside = root.path().join("outside.md");
    std::fs::write(&outside, "secret outside").unwrap();
    std::os::unix::fs::symlink(&outside, root.path().join("kb/files/link.md")).unwrap();
    let db = rusqlite::Connection::open(root.path().join("kb/kb_meta.db")).unwrap();
    db.execute("UPDATE files SET path=?1", [outside.to_str().unwrap()])
        .unwrap();
    drop(db);
    let (plugin, _) = runtime(root.path(), json!({}));
    let result = call(
        &plugin,
        root.path(),
        "search_knowledge_base",
        json!({"query":"secret"}),
        false,
    )
    .await
    .unwrap();
    assert_eq!(result["total_matches"], 0);
    for name in ["../outside.md", "link.md"] {
        assert!(call(
            &plugin,
            root.path(),
            "read_knowledge_base_file",
            json!({"file_name":name}),
            false
        )
        .await
        .is_err());
        assert!(call(
            &plugin,
            root.path(),
            "remove_knowledge_base_file",
            json!({"file_name":name}),
            true
        )
        .await
        .is_err());
    }
    assert_eq!(std::fs::read_to_string(outside).unwrap(), "secret outside");
}

/// 【知识库权限测试】【无来源授权】普通插件命令不能从参数自行取得输入文件权限
/// @returns 无；未授权单文件导入返回错误且没有正文
#[tokio::test]
async fn knowledge_normal_command_cannot_invent_explicit_input_grants() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("outside.md");
    std::fs::write(&source, "ungranted").unwrap();
    let (plugin, _) = runtime(root.path(), json!({}));
    assert!(command(&plugin, root.path(), "add", json!({"path":source}))
        .await
        .is_err());
    assert_eq!(files(root.path()), json!({}));
}
