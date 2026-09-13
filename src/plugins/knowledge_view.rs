use super::{
    commands::{run_installed, PluginCommand},
    discovery::find_optional,
};
use crate::{config::AppConfig, paths::SaiPaths};
use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

/// 【知识库界面】【文件投影】界面只使用插件返回的名称和字节数
#[derive(Deserialize)]
pub(crate) struct FileRecord {
    pub name: String,
    pub size_bytes: u64,
}

/// 【知识库界面】【命令适配】把显式用户操作交给同一受限插件入口
/// @param paths 应用目录；config 为配置；command 为插件命令；arguments 为结构化参数
/// @returns 原始命令输出
async fn call(
    paths: &SaiPaths,
    config: &AppConfig,
    command: &'static str,
    arguments: Value,
) -> Result<String> {
    run_installed(
        config,
        paths,
        PluginCommand {
            plugin: "knowledge-base",
            command,
            arguments,
            allow_writes: true,
        },
    )
    .await
}

/// 【知识库界面】【列表投影】不在 Rust 中读取知识库文件或数据库
/// @param paths 应用目录；config 为当前配置
/// @returns Lua 命令返回的文件列表
pub(crate) async fn list(paths: &SaiPaths, config: &AppConfig) -> Result<Vec<FileRecord>> {
    if !find_optional(config, paths, "knowledge-base")?.is_some_and(|plugin| plugin.setting.enabled)
    {
        return Ok(Vec::new());
    }
    Ok(serde_json::from_str(
        &call(paths, config, "list", json!({"format":"json"})).await?,
    )?)
}

/// 【知识库界面】【统计投影】复用插件统计格式
/// @param paths 应用目录；config 为当前配置
/// @returns 统计 JSON
pub(crate) async fn stats(paths: &SaiPaths, config: &AppConfig) -> Result<Value> {
    if !find_optional(config, paths, "knowledge-base")?.is_some_and(|plugin| plugin.setting.enabled)
    {
        return Ok(json!({
            "ok": true, "available": false, "root": "", "files_dir": "",
            "files": 0, "total_size_kb": 0, "semantic_chunks": 0,
            "embedding_enabled": false, "embedding_provider_id": "", "embedding_model": ""
        }));
    }
    Ok(serde_json::from_str(
        &call(paths, config, "stats", json!({})).await?,
    )?)
}

/// 【知识库界面】【显式导入】通过普通命令导入文件或目录，来源读取遵守插件授权
/// @param paths 应用目录；config 为配置；source 为用户选择的文件或目录
/// @returns 实际成功导入数量
pub(crate) async fn add(paths: &SaiPaths, config: &AppConfig, source: &Path) -> Result<usize> {
    let source = source.to_str().context("source path must be UTF-8")?;
    let text = call(paths, config, "add", json!({"path":source,"format":"json"})).await?;
    Ok(serde_json::from_str::<Vec<String>>(&text)?.len())
}

/// 【知识库界面】【确认后删除】由插件完成文件、元数据和语义块删除
/// @param paths 应用目录；config 为配置；name 为用户确认的相对名称
/// @returns 已完成的命令输出
pub(crate) async fn remove(paths: &SaiPaths, config: &AppConfig, name: &str) -> Result<String> {
    call(paths, config, "remove", json!({"file":name})).await
}
