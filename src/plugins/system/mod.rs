mod files;
pub(super) mod paths;
mod process;
mod process_group;
#[cfg(windows)]
mod process_windows_paths;

pub(super) use files::{file_info, read_directory, read_text};
pub(super) use process::{execute, execute_workspace};

use anyhow::Result;
use sai_plugin_runtime::Capabilities;

/// 【插件系统】【环境读取】读取已授权变量，未定义的变量不作为执行错误。
/// @param name 精确变量名；capabilities 为当前插件有效授权
/// @returns UTF-8 环境值或 None
pub(super) fn environment(name: &str, capabilities: &Capabilities) -> Result<Option<String>> {
    capabilities.system.authorize_environment(name)?;
    match std::env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(error.into()),
    }
}
