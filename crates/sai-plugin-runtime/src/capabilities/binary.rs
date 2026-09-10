use super::system::validate_read_path;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// 【插件二进制】【独立授权】公开下载、文件输出和图片展示分别授权。
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct BinaryCapabilities {
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub public_downloads: bool,
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub write_paths: BTreeSet<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub display_images: bool,
}

impl BinaryCapabilities {
    /// 【插件二进制】【空授权】检查是否没有任何二进制副作用能力。
    /// @returns 全部能力关闭时为 true
    pub fn is_empty(&self) -> bool {
        !self.public_downloads && self.write_paths.is_empty() && !self.display_images
    }

    /// 【插件二进制】【声明校验】限制输出目录数量并复用规范路径规则。
    /// @returns 声明合法时成功
    pub fn validate(&self) -> Result<()> {
        if self.write_paths.len() > 64 {
            bail!("plugin binary write paths exceed 64 entries");
        }
        for path in &self.write_paths {
            validate_read_path(path)?;
        }
        Ok(())
    }

    /// 【插件二进制】【授权交集】只保留声明和用户授权共同允许的能力。
    /// @param granted 用户明确授予的能力
    /// @returns 不会扩大输出目录或副作用的有效授权
    pub fn intersection(&self, granted: &Self) -> Self {
        Self {
            public_downloads: self.public_downloads && granted.public_downloads,
            write_paths: self
                .write_paths
                .intersection(&granted.write_paths)
                .cloned()
                .collect(),
            display_images: self.display_images && granted.display_images,
        }
    }

    /// 【插件二进制】【授权范围】检查显式授权是否全部属于包声明。
    /// @param declared 包声明的完整能力
    /// @returns 当前授权是声明子集时为 true
    pub fn is_subset(&self, declared: &Self) -> bool {
        (!self.public_downloads || declared.public_downloads)
            && (!self.display_images || declared.display_images)
            && self.write_paths.is_subset(&declared.write_paths)
    }

    /// 【插件二进制】【写入前置】先验证路径、输出授权和宿主写入状态。
    /// @param path 请求路径；allow_writes 为宿主确认的权限
    /// @returns 可继续执行目录句柄校验时成功
    pub fn check_write(&self, path: &str, allow_writes: bool) -> Result<()> {
        validate_read_path(path)?;
        if self.write_paths.is_empty() {
            bail!("plugin binary file writing is not allowed");
        }
        if !allow_writes {
            bail!("read-only plugin callback cannot write a binary file");
        }
        Ok(())
    }
}
