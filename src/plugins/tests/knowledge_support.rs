use crate::{config::AppConfig, paths::SaiPaths, plugins::discovery::PluginDescriptor};
use sai_plugin_runtime::{
    host::PluginHost, Capabilities, InvocationContext, PluginPackage, PluginRuntime,
};
use serde_json::{json, Value};
use std::{path::Path, sync::Arc};

pub(super) const META_SCHEMA: &str = "CREATE TABLE files (name TEXT PRIMARY KEY, path TEXT NOT NULL, size_bytes INTEGER NOT NULL, mtime REAL NOT NULL, content_sha256 TEXT NOT NULL, updated_at REAL NOT NULL)";
pub(super) const SEMANTIC_SCHEMA: &str = "CREATE TABLE semantic_chunks (id INTEGER PRIMARY KEY AUTOINCREMENT, provider_id TEXT NOT NULL, model TEXT NOT NULL, file_name TEXT NOT NULL, content_sha256 TEXT NOT NULL, chunk_index INTEGER NOT NULL, start_char INTEGER NOT NULL, end_char INTEGER NOT NULL, text TEXT NOT NULL, embedding_json TEXT NOT NULL, created_at REAL NOT NULL); CREATE INDEX idx_semantic_file ON semantic_chunks(file_name, content_sha256)";

/// 【知识库测试】【旧文件装配】直接创建冻结原结构，避免用待测 Lua 生成预期输入
/// @param root 隔离目录；files 为相对文件名和原文；initialized 为是否创建空库
/// @returns 无；连接在调用结束前全部关闭
pub(super) fn seed(root: &Path, files: &Value, initialized: bool) {
    if !initialized && files.as_object().is_none_or(|files| files.is_empty()) {
        return;
    }
    let directory = root.join("kb");
    std::fs::create_dir_all(directory.join("files")).unwrap();
    let meta = rusqlite::Connection::open(directory.join("kb_meta.db")).unwrap();
    meta.execute_batch(META_SCHEMA).unwrap();
    let semantic = rusqlite::Connection::open(directory.join("semantic_index.db")).unwrap();
    semantic.execute_batch(SEMANTIC_SCHEMA).unwrap();
    if let Some(files) = files.as_object() {
        for (name, text) in files {
            let text = text.as_str().unwrap();
            let path = directory.join("files").join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, text).unwrap();
            meta.execute(
                "INSERT INTO files VALUES (?1,?2,?3,?4,?5,?6)",
                rusqlite::params![
                    name,
                    path.display().to_string(),
                    text.len() as i64,
                    10.0,
                    super::binary_conditional_support::digest(text.as_bytes()),
                    20.0
                ],
            )
            .unwrap();
        }
    }
}

/// 【知识库测试】【正式描述】旧开关、配置和能力经过真实兼容层
/// @param root 测试目录；config 为主配置；extra 为显式包设置
/// @returns 实际内置包的固定描述符
pub(super) fn descriptor(root: &Path, config: &AppConfig, extra: Value) -> PluginDescriptor {
    let paths = SaiPaths::for_tests(root);
    let mut descriptor = crate::plugins::discover(config, &paths)
        .plugins
        .into_iter()
        .find(|p| p.package.manifest.id == "knowledge-base")
        .unwrap();
    descriptor.setting.settings =
        json!({"data_dir":root.join("kb"),"input_paths":[root.join("input")]});
    descriptor
        .setting
        .settings
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    descriptor.refresh_compatibility(config, &paths).unwrap();
    descriptor
}

/// 【知识库测试】【完整实例】追加测试入口但不替换发布业务，授权可单独收窄
/// @param root 目录；config 为主配置；extra 为设置；host 为宿主；suffix 为测试装配；change 为授权修改
/// @returns 独立 Lua 运行时
pub(super) fn configured(
    root: &Path,
    config: &AppConfig,
    extra: Value,
    host: Arc<dyn PluginHost>,
    suffix: &str,
    change: impl FnOnce(&mut Capabilities),
) -> PluginRuntime {
    let descriptor = descriptor(root, config, extra);
    let mut package = descriptor.runtime_package();
    let mut sources = package.sources().clone();
    sources.get_mut("init.lua").unwrap().push_str(suffix);
    let mut grants = descriptor.grants();
    change(&mut grants);
    package = PluginPackage::new(package.manifest, sources).unwrap();
    PluginRuntime::load(package, descriptor.settings().clone(), grants, host).unwrap()
}

/// 【知识库测试】【默认实例】实际文件与 SQLite 接口由正式宿主提供
/// @param root 隔离目录；extra 为包设置
/// @returns 实例及可注入故障的宿主
pub(super) fn runtime(
    root: &Path,
    extra: Value,
) -> (PluginRuntime, Arc<super::knowledge_host::KnowledgeHost>) {
    let host = super::knowledge_host::KnowledgeHost::new(root);
    (
        configured(root, &AppConfig::default(), extra, host.clone(), "", |_| {}),
        host,
    )
}

/// 【知识库测试】【可信调用】工作目录和写入权限从测试宿主提供
/// @param root 工作目录；writes 为实际许可
/// @returns 当前调用上下文
pub(super) fn context(root: &Path, writes: bool) -> InvocationContext {
    InvocationContext {
        workdir: root.display().to_string(),
        allow_writes: writes,
        session_id: "knowledge-test".into(),
        ..Default::default()
    }
}

/// 【知识库测试】【正式工具】调用已发布工具并保留 JSON 或纯文本结果
/// @param runtime 实例；root 为目录；name 为工具；args 为参数；writes 为写入许可
/// @returns 业务值或完整错误
pub(super) async fn call(
    runtime: &PluginRuntime,
    root: &Path,
    name: &str,
    args: Value,
    writes: bool,
) -> anyhow::Result<Value> {
    let result = runtime.call_tool(name, args, context(root, writes)).await?;
    Ok(serde_json::from_str(&result).unwrap_or(Value::String(result)))
}

/// 【知识库测试】【正式命令】参数为实例、目录、命令名和 JSON；返回完整原始输出
pub(super) async fn command(
    runtime: &PluginRuntime,
    root: &Path,
    name: &str,
    args: Value,
) -> anyhow::Result<String> {
    runtime
        .call_command(name, &args.to_string(), context(root, true))
        .await
}

/// 【知识库测试】【嵌入配置】输入无；返回只包含固定测试凭据的供应商和知识库设置
pub(super) fn embedding_config() -> AppConfig {
    let mut config = AppConfig::default();
    let mut provider = crate::config::ProviderConfig::default_openai();
    provider.id = "embedding-test".into();
    provider.base_url = "https://embedding.test/v1".into();
    provider.api_key = Some("fixture-embedding-key".into());
    config.providers = vec![provider];
    config.plugins.knowledge_base.embedding_enabled = true;
    config.plugins.knowledge_base.embedding_provider_id = "embedding-test".into();
    config.plugins.knowledge_base.embedding_model = "fixture-vector".into();
    config
}

/// 【知识库测试】【旧语义行】输入目录、文件名、正文和 JSON 向量；返回无，直接建立独立预期记录
pub(super) fn semantic_row(root: &Path, name: &str, text: &str, vector: &str) {
    let db = rusqlite::Connection::open(root.join("kb/semantic_index.db")).unwrap();
    db.execute("INSERT INTO semantic_chunks(provider_id,model,file_name,content_sha256,chunk_index,start_char,end_char,text,embedding_json,created_at) VALUES ('old-provider','old-model',?1,'old-digest',0,0,1,?2,?3,0)", rusqlite::params![name,text,vector]).unwrap();
}

/// 【知识库测试】【索引读取】输入目录；返回现有语义行的文件、正文、向量和摘要
pub(super) fn semantic_rows(root: &Path) -> Vec<(String, String, String, String)> {
    let db = rusqlite::Connection::open(root.join("kb/semantic_index.db")).unwrap();
    let mut query = db
        .prepare(
            "SELECT file_name,text,embedding_json,content_sha256 FROM semantic_chunks ORDER BY id",
        )
        .unwrap();
    query
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

/// 【知识库测试】【正文快照】递归读取知识库普通文件，不包含数据库或恢复记录
/// @param root 库根目录
/// @returns 相对文件名到正文的映射
pub(super) fn files(root: &Path) -> Value {
    let base = root.join("kb/files");
    let mut result = serde_json::Map::new();
    let mut stack = vec![base.clone()];
    while let Some(directory) = stack.pop() {
        if !directory.exists() {
            continue;
        }
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                result.insert(
                    path.strip_prefix(&base)
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .replace('\\', "/"),
                    json!(std::fs::read_to_string(path).unwrap()),
                );
            }
        }
    }
    Value::Object(result)
}
