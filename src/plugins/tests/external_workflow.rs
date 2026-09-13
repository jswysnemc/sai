use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::permission::{PermissionProfile, PermissionProfileMode};
use crate::plugins::config::load_config;
use crate::plugins::discovery::find;
use crate::plugins::private::PrivatePluginHost;
use crate::plugins::registry::register_descriptor;
use crate::plugins::{self, GrantChanges, GrantUpdate, PluginSource};
use crate::tools::ToolRegistry;
use anyhow::Result;
use sai_plugin_runtime::PluginPackage;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;

const ID: &str = "project-notes";
const TOOL: &str = "lua__project-notes__inspect";
const NOTE: &str = "# 项目笔记\n\n公开接口组合示例。\n";

struct Fixture {
    root: tempfile::TempDir,
    paths: SaiPaths,
    source: PathBuf,
    config: AppConfig,
}

impl Fixture {
    /// 【外部扩展测试】【独立安装】复制可发布示例到仓库外并使用正常安装事务
    /// @returns 带独立配置、源码和项目笔记的临时环境
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(&root.path().join("app"));
        let source = root.path().join("source");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::create_dir_all(root.path().join("notes")).unwrap();
        std::fs::write(root.path().join("notes/README.md"), NOTE).unwrap();
        let package = PluginPackage::from_directory(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/lua-plugins/project-notes"),
        )
        .unwrap();
        std::fs::write(
            source.join("sai-plugin.json"),
            serde_json::to_vec(&package.manifest).unwrap(),
        )
        .unwrap();
        for (path, content) in package.sources() {
            std::fs::write(source.join(path), content).unwrap();
        }
        plugins::install(&source, &paths, false).unwrap();
        Self {
            root,
            paths,
            source,
            config: AppConfig::default(),
        }
    }

    /// 【外部扩展测试】【显式授权】通过管理接口修改同一普通外部包的权限
    /// @param update 要保存的能力变更
    /// @returns 无，保存失败时终止测试
    fn enable(&self, update: GrantUpdate) {
        plugins::set_enabled(&self.config, &self.paths, ID, true, update).unwrap();
    }

    /// 【外部扩展测试】【真实宿主】加载已安装快照并绑定实际文件和私有存储宿主
    /// @returns 使用独立会话且默认只读的工具注册表
    fn registry(&self) -> ToolRegistry {
        let descriptor = find(&self.config, &self.paths, ID).unwrap();
        assert!(matches!(descriptor.source, PluginSource::Installed(_)));

        let mut registry = ToolRegistry::new();
        if descriptor.setting.enabled {
            let host = PrivatePluginHost::for_descriptor(&self.paths, &descriptor).unwrap();
            register_descriptor(&mut registry, descriptor, Arc::new(host), false).unwrap();
        }
        registry.start_plugin_session("external-notes").unwrap();
        registry.set_permission_profile(PermissionProfile::new(
            PermissionProfileMode::Plan,
            self.root.path().to_path_buf(),
            None,
        ));
        registry
    }

    /// 【外部扩展测试】【工具调用】在独立任务工作目录内执行正式工具路径
    /// @param registry 当前实例；arguments 为工具参数
    /// @returns 解析后的工具结果或原始调用错误
    async fn inspect(&self, registry: &ToolRegistry, arguments: Value) -> Result<Value> {
        let text = crate::runtime_cwd::scope(
            self.root.path().to_path_buf(),
            registry.call(TOOL, &arguments.to_string()),
        )
        .await?;
        Ok(serde_json::from_str(&text)?)
    }

    /// 【外部扩展测试】【命令调用】复用 CLI 和 TUI 的命令工具适配以及权限流程
    /// @param registry 当前注册表；name 为命令名称
    /// @returns 命令的 JSON 结果或授权错误
    async fn command(&self, registry: &mut ToolRegistry, name: &str) -> Result<Value> {
        let command = registry.plugin_command(ID, name)?;
        let name = command.name.clone();
        registry.register(command);
        let text = crate::runtime_cwd::scope(
            self.root.path().to_path_buf(),
            registry.call(&name, r#"{"arguments":""}"#),
        )
        .await?;
        Ok(serde_json::from_str(&text)?)
    }
}

/// 【外部扩展测试】【公共接口组合】普通包独立取得读取和存储授权，观察真实工具结果
/// @returns 无，写入还必须通过宿主计划模式检查
#[tokio::test]
async fn example_composes_public_services_with_explicit_grants() {
    let fixture = Fixture::new();
    assert!(!fixture.registry().contains(TOOL));
    fixture.enable(GrantUpdate::Keep);
    let denied = fixture.registry();
    assert!(fixture.inspect(&denied, json!({})).await.is_err());

    fixture.enable(GrantUpdate::Changes(GrantChanges {
        read_paths: Some(["notes".into()].into()),
        ..Default::default()
    }));
    let mut readonly = fixture.registry();
    let result = fixture.inspect(&readonly, json!({})).await.unwrap();
    assert_eq!(result["bytes"], NOTE.len());
    assert_eq!(
        result["sha256"],
        format!("{:x}", <sha2::Sha256 as sha2::Digest>::digest(NOTE))
    );
    assert_eq!(
        fixture.command(&mut readonly, "stats").await.unwrap()["inspections"],
        1
    );
    assert!(fixture.command(&mut readonly, "latest").await.is_err());

    fixture.enable(GrantUpdate::Changes(GrantChanges {
        plugin_storage: Some(true),
        ..Default::default()
    }));
    let mut writable = fixture.registry();
    assert_eq!(
        fixture.command(&mut writable, "latest").await.unwrap(),
        json!({"record":null})
    );
    assert!(fixture.command(&mut writable, "remember").await.is_err());
    writable.set_permission_mode(PermissionProfileMode::Yolo);
    let saved = fixture.command(&mut writable, "remember").await.unwrap();
    assert_eq!(saved["sha256"], result["sha256"]);
    assert_eq!(saved["schema_version"], 1);
    assert!(saved["checked_at"].as_str().unwrap().contains('T'));
    let mut next_process = fixture.registry();
    assert_eq!(
        fixture.command(&mut next_process, "latest").await.unwrap()["record"],
        saved
    );
    assert_eq!(
        std::fs::read_to_string(fixture.root.path().join("notes/README.md")).unwrap(),
        NOTE
    );
}

/// 【外部扩展测试】【快照重载】沿用未变实例，源码、设置或授权改变后更新工具和事件快照
/// @returns 无，旧实例完成原调用且更新不扩大授权
#[tokio::test]
async fn example_updates_and_reloads_without_builtin_registration() {
    let fixture = Fixture::new();
    fixture.enable(GrantUpdate::Declared);
    let first = fixture.registry();
    fixture.inspect(&first, json!({})).await.unwrap();
    let mut unchanged = fixture.registry();
    unchanged.continue_plugin_session(&first);
    assert_eq!(
        fixture.command(&mut unchanged, "stats").await.unwrap()["inspections"],
        1
    );

    plugins::configure(
        &fixture.config,
        &fixture.paths,
        ID,
        json!({"preview_chars":3}),
    )
    .unwrap();
    let mut configured = fixture.registry();
    configured.continue_plugin_session(&unchanged);
    assert_eq!(
        fixture.command(&mut configured, "stats").await.unwrap()["inspections"],
        0
    );
    let shorter = fixture.inspect(&configured, json!({})).await.unwrap();
    assert_ne!(
        shorter["preview"],
        fixture.inspect(&first, json!({})).await.unwrap()["preview"]
    );

    let manifest_path = fixture.source.join("sai-plugin.json");
    let mut manifest: Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
    manifest["version"] = json!("0.2.0");
    manifest["capabilities"]["system"]["environment"] = json!(["LANG"]);
    std::fs::write(&manifest_path, manifest.to_string()).unwrap();
    plugins::install(&fixture.source, &fixture.paths, true).unwrap();
    let descriptor = find(&fixture.config, &fixture.paths, ID).unwrap();
    assert_eq!(descriptor.package.manifest.version, "0.2.0");
    assert_eq!(descriptor.settings(), &json!({"preview_chars":3}));
    assert!(descriptor
        .capabilities()
        .intersection(&descriptor.grants())
        .system
        .environment
        .is_empty());
    let mut updated = fixture.registry();
    updated.continue_plugin_session(&configured);
    assert_eq!(
        fixture.command(&mut updated, "stats").await.unwrap()["inspections"],
        0
    );
    fixture.inspect(&updated, json!({})).await.unwrap();

    fixture.enable(GrantUpdate::Changes(GrantChanges {
        read_paths: Some(Default::default()),
        ..Default::default()
    }));
    let mut revoked = fixture.registry();
    revoked.continue_plugin_session(&updated);
    assert!(fixture.inspect(&revoked, json!({})).await.is_err());
    assert!(fixture.inspect(&updated, json!({})).await.is_ok());
    plugins::set_enabled(
        &fixture.config,
        &fixture.paths,
        ID,
        false,
        GrantUpdate::Keep,
    )
    .unwrap();
    let mut disabled = fixture.registry();
    disabled.continue_plugin_session(&revoked);
    assert!(!disabled.contains(TOOL));
    assert!(disabled.plugin_command(ID, "inspect").is_err());
    plugins::remove(&fixture.config, &fixture.paths, ID).unwrap();
    let setting = load_config(&fixture.paths).unwrap().plugins[ID].clone();
    assert!(!setting.enabled);
    assert_eq!(setting.grants.unwrap(), Default::default());
}

/// 【外部扩展测试】【完整读取边界】拒绝截断、非法文本和越界路径，错误设置不覆盖已保存值
/// @returns 无，Schema 和设置检查不会产生额外读取或写入
#[tokio::test]
async fn example_reports_input_and_file_boundaries() {
    let fixture = Fixture::new();
    fixture.enable(GrantUpdate::Declared);
    let note = fixture.root.path().join("notes/README.md");
    let registry = fixture.registry();
    assert!(fixture
        .inspect(&registry, json!({"path":"elsewhere"}))
        .await
        .is_err());
    std::fs::write(&note, vec![b'x'; 65537]).unwrap();
    let error = fixture.inspect(&registry, json!({})).await.unwrap_err();
    assert!(format!("{error:#}").contains("exceeds 65536 bytes"));
    std::fs::write(&note, [0xff]).unwrap();
    assert!(fixture.inspect(&registry, json!({})).await.is_err());

    let before = std::fs::read(fixture.paths.config_dir.join("plugins.jsonc")).unwrap();
    for settings in [
        json!({"preview_chars":false}),
        json!({"preview_chars":0}),
        json!({"path":""}),
        json!({"unexpected":true}),
    ] {
        assert!(plugins::configure(&fixture.config, &fixture.paths, ID, settings).is_err());
        assert_eq!(
            std::fs::read(fixture.paths.config_dir.join("plugins.jsonc")).unwrap(),
            before
        );
    }
    std::fs::write(fixture.root.path().join("outside.txt"), "outside notes").unwrap();
    plugins::configure(
        &fixture.config,
        &fixture.paths,
        ID,
        json!({"path":"outside.txt"}),
    )
    .unwrap();
    assert!(fixture
        .inspect(&fixture.registry(), json!({}))
        .await
        .is_err());
}
