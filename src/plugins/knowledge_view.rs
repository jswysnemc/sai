use super::commands::{run_bundled, BundledCommand};
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
/// @param paths 应用目录；config 为配置；command 为内置命令；arguments 为结构化参数
/// @returns 原始命令输出
async fn call(
    paths: &SaiPaths,
    config: &AppConfig,
    command: &'static str,
    arguments: Value,
) -> Result<String> {
    run_bundled(
        config,
        paths,
        BundledCommand {
            plugin: "knowledge-base",
            command,
            arguments,
            input: None,
            allow_writes: true,
        },
    )
    .await
}

/// 【知识库界面】【列表投影】不在 Rust 中读取知识库文件或数据库
/// @param paths 应用目录；config 为当前配置
/// @returns Lua 命令返回的文件列表
pub(crate) async fn list(paths: &SaiPaths, config: &AppConfig) -> Result<Vec<FileRecord>> {
    Ok(serde_json::from_str(
        &call(paths, config, "list", json!({"format":"json"})).await?,
    )?)
}

/// 【知识库界面】【统计投影】复用插件统计格式
/// @param paths 应用目录；config 为当前配置
/// @returns 统计 JSON
pub(crate) async fn stats(paths: &SaiPaths, config: &AppConfig) -> Result<Value> {
    Ok(serde_json::from_str(
        &call(paths, config, "stats", json!({})).await?,
    )?)
}

/// 【知识库界面】【显式来源】只为用户选择的文件或目录授予临时读取权限
/// @param paths 应用目录；config 为配置；source 为已确认输入
/// @returns 实际成功导入数量
pub(crate) async fn add(paths: &SaiPaths, config: &AppConfig, source: &Path) -> Result<usize> {
    let name = source
        .file_name()
        .and_then(|name| name.to_str())
        .context("source path has no valid name")?;
    let canonical = dunce::canonicalize(source)?;
    let text = run_bundled(
        config,
        paths,
        BundledCommand {
            plugin: "knowledge-base",
            command: "add",
            arguments: json!({"path":canonical,"name":name,"format":"json"}),
            input: Some(canonical),
            allow_writes: true,
        },
    )
    .await?;
    Ok(serde_json::from_str::<Vec<String>>(&text)?.len())
}

/// 【知识库界面】【确认后删除】由插件完成文件、元数据和语义块删除
/// @param paths 应用目录；config 为配置；name 为用户确认的相对名称
/// @returns 已完成的命令输出
pub(crate) async fn remove(paths: &SaiPaths, config: &AppConfig, name: &str) -> Result<String> {
    call(paths, config, "remove", json!({"file":name})).await
}
