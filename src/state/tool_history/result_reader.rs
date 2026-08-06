use crate::state::StateStore;
use anyhow::{bail, Context, Result};
use std::path::{Component, Path, PathBuf};

/// 复用已规范化会话根目录的工具结果读取器。
pub(crate) struct ToolResultRefReader {
    state_root: PathBuf,
}

impl StateStore {
    /// 创建当前会话的工具结果读取器，根目录只解析一次。
    ///
    /// 返回:
    /// - 可安全读取会话内结果引用的读取器
    pub(crate) fn tool_result_ref_reader(&self) -> Result<ToolResultRefReader> {
        ToolResultRefReader::new(&self.state_dir)
    }

    /// 读取当前会话目录中的完整工具结果引用。
    ///
    /// 参数:
    /// - `result_ref`: 相对于当前会话目录的工具结果引用
    ///
    /// 返回:
    /// - 完整工具结果文本；引用越界、缺失或不可读时返回错误
    #[cfg(test)]
    pub(crate) fn read_tool_result_ref(&self, result_ref: &str) -> Result<String> {
        self.tool_result_ref_reader()?.read(result_ref)
    }
}

impl ToolResultRefReader {
    /// 使用会话状态目录创建读取器。
    ///
    /// 参数:
    /// - `state_dir`: 当前会话状态目录
    ///
    /// 返回:
    /// - 已规范化根目录的读取器
    fn new(state_dir: &Path) -> Result<Self> {
        let state_root = std::fs::canonicalize(state_dir).context("无法解析当前会话状态目录")?;
        Ok(Self { state_root })
    }

    /// 读取完整工具结果。
    ///
    /// 参数:
    /// - `result_ref`: 会话内相对结果引用
    ///
    /// 返回:
    /// - 完整结果文本
    #[cfg(test)]
    pub(crate) fn read(&self, result_ref: &str) -> Result<String> {
        let result_path = self.resolve(result_ref)?;
        std::fs::read_to_string(&result_path)
            .with_context(|| format!("无法读取完整工具结果引用: {result_ref}"))
    }

    /// 在字节预算内读取工具结果，超出限制时不打开文件。
    ///
    /// 参数:
    /// - `result_ref`: 会话内相对结果引用
    /// - `max_bytes`: 本次允许读取的最大字节数
    ///
    /// 返回:
    /// - 预算内返回完整文本；超出预算返回空值
    pub(crate) fn read_with_limit(
        &self,
        result_ref: &str,
        max_bytes: usize,
    ) -> Result<Option<String>> {
        let result_path = self.resolve(result_ref)?;
        let metadata = std::fs::metadata(&result_path)
            .with_context(|| format!("无法读取完整工具结果元数据: {result_ref}"))?;
        if metadata.len() > max_bytes as u64 {
            return Ok(None);
        }
        std::fs::read_to_string(&result_path)
            .with_context(|| format!("无法读取完整工具结果引用: {result_ref}"))
            .map(Some)
    }

    /// 校验并解析会话内结果引用。
    ///
    /// 参数:
    /// - `result_ref`: 会话内相对结果引用
    ///
    /// 返回:
    /// - 规范化后的安全文件路径
    fn resolve(&self, result_ref: &str) -> Result<PathBuf> {
        let reference = Path::new(result_ref);
        if result_ref.trim().is_empty()
            || reference.is_absolute()
            || reference
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            bail!("工具结果引用必须是会话目录内的普通相对路径");
        }

        let result_path = std::fs::canonicalize(self.state_root.join(reference))
            .with_context(|| format!("完整工具结果引用不存在: {result_ref}"))?;
        if !result_path.starts_with(&self.state_root) || !result_path.is_file() {
            bail!("工具结果引用超出当前会话目录: {result_ref}");
        }
        Ok(result_path)
    }
}
