use super::{DirectoryListing, FileInfo, FileReadRequest, FileText, ProcessOutput, ProcessRequest};
use crate::Capabilities;
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 【插件】【私有状态】按插件与宿主会话隔离的原子键值操作，null 表示删除。
#[derive(Clone, Debug)]
pub enum StorageRequest {
    Get {
        key: String,
    },
    Set {
        key: String,
        value: Value,
    },
    CompareExchange {
        key: String,
        expected: Value,
        value: Value,
    },
}

/// 【插件】【归档请求】只接收有界 tar.gz 下载，不开放任意二进制文件写入。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArchiveRequest {
    pub url: String,
    pub destination: String,
    pub max_bytes: usize,
    pub max_unpacked_bytes: u64,
    pub max_entries: usize,
    pub timeout_ms: u64,
}

/// 【插件】【状态键】键只参与摘要计算，不作为文件系统路径使用。
/// @param key 非空、无控制字符的有界文本
/// @returns 可用作私有记录或工作目录键时成功
pub fn validate_storage_key(key: &str) -> Result<()> {
    if key.is_empty() || key.len() > 256 || key.chars().any(char::is_control) {
        anyhow::bail!("plugin private key must contain 1-256 bytes without control characters");
    }
    Ok(())
}

/// 【插件】【私有路径】仅允许规范相对路径，目录根使用点号表示。
/// @param path 相对文件或目录路径
/// @returns 没有父目录、设备名或平台歧义时成功
pub fn validate_workspace_path(path: &str) -> Result<()> {
    if path == "." {
        return Ok(());
    }
    crate::manifest::validate_relative_file(path)
}

/// 【插件】【私有工作目录】宿主创建并持有目录；路径参数必须相对于该目录。
#[async_trait]
pub trait PluginWorkspace: Send + Sync {
    /// 【插件】【显示路径】提供可用于报告的绝对路径，不授予额外文件权限。
    /// @returns 当前私有目录的显示路径
    fn path(&self) -> String;

    /// 【插件】【目录正文】读取私有目录内的有界文本。
    /// @param request 相对路径及文本限制
    /// @returns 文本及截断标记
    async fn read_text(&self, request: FileReadRequest) -> Result<FileText>;

    /// 【插件】【目录列表】读取私有目录内的直接子项。
    /// @param path 相对目录；max_entries 为条数上限
    /// @returns 条目及截断标记
    async fn read_directory(&self, path: String, max_entries: usize) -> Result<DirectoryListing>;

    /// 【插件】【文件属性】查询私有目录内的文件或目录。
    /// @param path 相对路径
    /// @returns 属性，不存在时为 None
    async fn file_info(&self, path: String) -> Result<Option<FileInfo>>;

    /// 【插件】【归档展开】下载已授权来源的归档，完整校验后发布到新子目录。
    /// @param request 下载与解压限制；capabilities 为本插件有效授权
    /// @returns 成功时无返回值；失败不得发布部分目录
    async fn extract_archive(
        &self,
        request: ArchiveRequest,
        capabilities: Capabilities,
    ) -> Result<()>;

    /// 【插件】【目录进程】在已验证的私有子目录执行明确授权的模板。
    /// @param request 模板与参数；directory 为相对目录；capabilities 为授权；allow_writes 为调用权限
    /// @returns 有界进程结果，取消时回收进程
    async fn process(
        &self,
        request: ProcessRequest,
        directory: String,
        capabilities: Capabilities,
        allow_writes: bool,
    ) -> Result<ProcessOutput>;
}
