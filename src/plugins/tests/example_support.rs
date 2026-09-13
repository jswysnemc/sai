use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::plugins::discovery::find;
use crate::plugins::registry::register_descriptor;
use crate::plugins::{self, PluginSource};
use crate::tools::ToolRegistry;
use sai_plugin_runtime::host::PluginHost;
use sai_plugin_runtime::{PluginPackage, PluginRuntime};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

pub(super) const EXTRACTED_IDS: &[&str] = &[
    "alarm",
    "reply-notification",
    "memes",
    "todo",
    "knowledge-base",
    "diagnostic-evidence",
    "image-display",
    "image-generation",
    "input-method-investigation",
    "linux-game-investigation",
    "package-advisor",
    "web-images",
    "archlinux",
    "deepseek-status",
    "exchange-rate",
    "fcitx-wiki",
    "hash-codec",
    "linux-game-signals",
    "moegirl",
    "online-man",
    "protondb",
    "weather",
    "web-fetch",
    "web-search",
    "xuanxue",
];

/// 【示例插件测试】【源码目录】只定位独立示例，不回退到内置资源
/// @param id 示例包标识
/// @returns 仓库内示例源码的绝对路径
pub(super) fn directory(id: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples/lua-plugins")
        .join(id)
}

/// 【示例插件测试】【源码快照】使用普通外部包读取器加载唯一业务源码
/// @param id 示例包标识
/// @returns 清单与 Lua 源码快照
pub(super) fn package(id: &str) -> PluginPackage {
    PluginPackage::from_directory(&directory(id)).unwrap()
}

/// 【示例插件测试】【普通安装】经过实际安装事务创建缺省禁用且无授权的包
/// @param id 示例包标识；paths 为隔离应用目录
/// @returns 无；安装错误终止当前测试
pub(in crate::plugins) fn install(id: &str, paths: &SaiPaths) {
    plugins::install(&directory(id), paths, false).unwrap();
}

/// 【示例插件测试】【派生清单】在仓库外调整测试清单，再经过普通安装入口
/// @param id 示例标识；paths 为隔离目录；change 为明确的测试能力变更
/// @returns 无；不修改正式示例源码或自动授权
pub(super) fn install_custom(
    id: &str,
    paths: &SaiPaths,
    change: impl FnOnce(&mut sai_plugin_runtime::PluginManifest),
) {
    let mut package = package(id);
    change(&mut package.manifest);
    let source = tempfile::tempdir().unwrap();
    std::fs::write(
        source.path().join("sai-plugin.json"),
        package.manifest_json().unwrap(),
    )
    .unwrap();
    for (path, contents) in package.sources() {
        let destination = source.path().join(path);
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
        std::fs::write(destination, contents).unwrap();
    }
    plugins::install(source.path(), paths, false).unwrap();
}

/// 【示例插件测试】【依赖准备】普通安装测试依赖并显式授予其声明能力
/// @param id 示例包标识；config 为主配置；paths 为隔离应用目录
/// @returns 无；重复准备复用已安装源码，授权行为始终明确
pub(super) fn install_enabled(id: &str, config: &AppConfig, paths: &SaiPaths) {
    if !paths.config_dir.join("plugins").join(id).exists() {
        install(id, paths);
    }
    plugins::set_enabled(config, paths, id, true, plugins::GrantUpdate::Declared).unwrap();
}

/// 【示例插件测试】【业务回归】显式授予测试清单能力以复用冻结业务样本
/// @param id 示例包标识；host 为可观察的测试宿主
/// @returns 从示例快照加载的独立运行时
pub(super) fn runtime(id: &str, host: Arc<dyn PluginHost>) -> PluginRuntime {
    let package = package(id);
    let grants = package.manifest.capabilities.clone();
    PluginRuntime::load(package, json!({}), grants, host).unwrap()
}

/// 【示例插件测试】【安装后注册】验证普通身份并严格使用已保存的设置和授权
/// @param config 主配置；paths 为隔离目录；id 为包标识；host 为可观察宿主
/// @returns 只注册指定示例且启动独立会话的工具表
pub(super) fn registry(
    config: &AppConfig,
    paths: &SaiPaths,
    id: &str,
    host: Arc<dyn PluginHost>,
) -> ToolRegistry {
    let descriptor = find(config, paths, id).unwrap();
    assert!(matches!(descriptor.source, PluginSource::Installed(_)));

    let mut registry = ToolRegistry::new();
    if descriptor.setting.enabled {
        register_descriptor(&mut registry, descriptor, host, false).unwrap();
    }
    registry.start_plugin_session("example-lifecycle").unwrap();
    registry
}
