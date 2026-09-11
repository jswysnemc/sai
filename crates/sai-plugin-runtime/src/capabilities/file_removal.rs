use super::{validate_read_path, SystemCapabilities};
use crate::host::FileRemovalKind;
use anyhow::{bail, Result};
use std::collections::BTreeSet;

impl SystemCapabilities {
    /// 【插件文件】【授权选择】根据固定操作类型选择独立目录集合
    /// @param kind 永久删除或回收站操作
    /// @returns 对应有效目录声明的只读引用
    pub fn removal_paths(&self, kind: FileRemovalKind) -> &BTreeSet<String> {
        match kind {
            FileRemovalKind::Permanent => &self.remove_paths,
            FileRemovalKind::Trash => &self.trash_paths,
        }
    }

    /// 【插件文件】【删除前置校验】验证路径语法、独立能力及可信写入权限，目录归属由宿主复核
    /// @param path 请求路径；kind 为固定操作；allow_writes 为宿主交付的写入权限
    /// @returns 请求允许进入宿主目录检查时成功
    pub fn check_removal_request(
        &self,
        path: &str,
        kind: FileRemovalKind,
        allow_writes: bool,
    ) -> Result<()> {
        validate_read_path(path)?;
        if self.removal_paths(kind).is_empty() {
            bail!("plugin file removal is not allowed for this operation");
        }
        if !allow_writes {
            bail!("read-only plugin callback cannot remove or trash files");
        }
        Ok(())
    }
}
