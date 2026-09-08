use crate::Capabilities;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

pub const API_VERSION: u32 = 1;

/// 【插件】【清单】声明插件身份、入口和所需宿主能力。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PluginManifest {
    pub api_version: u32,
    pub id: String,
    pub version: String,
    pub name: String,
    pub description: String,
    pub entry: String,
    #[serde(default)]
    pub capabilities: Capabilities,
    #[serde(default)]
    pub limits: ExecutionLimits,
}

/// 【插件】【资源限制】限制单次回调的内存、指令、时长和结果大小。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExecutionLimits {
    pub memory_bytes: usize,
    pub instructions: u64,
    pub timeout_ms: u64,
    pub http_timeout_ms: u64,
    pub output_bytes: usize,
}

impl Default for ExecutionLimits {
    /// 返回运行时默认限制，无参数。
    fn default() -> Self {
        Self {
            memory_bytes: 16 * 1024 * 1024,
            instructions: 2_000_000,
            timeout_ms: 20_000,
            http_timeout_ms: 30_000,
            output_bytes: 1024 * 1024,
        }
    }
}

impl PluginManifest {
    /// 【插件】【清单校验】解析 JSON 清单并检查全部约束。
    /// @param text 清单 JSON 文本
    /// @returns 可加载的清单，非法字段或不支持版本返回错误
    pub fn parse(text: &str) -> Result<Self> {
        let manifest: Self = serde_json::from_str(text).context("invalid plugin manifest JSON")?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// 【插件】【清单校验】验证插件标识、API 版本、入口和资源上限。
    /// @returns 校验结果，不产生文件或网络副作用
    pub fn validate(&self) -> Result<()> {
        if self.api_version != API_VERSION {
            bail!(
                "unsupported plugin API version {}; expected {API_VERSION}",
                self.api_version
            );
        }
        validate_identifier(&self.id, 32).context("invalid plugin id")?;
        if windows_device_name(&self.id) {
            bail!("plugin id is a reserved Windows device name");
        }
        if self.version.trim().is_empty() || self.version.len() > 64 {
            bail!("plugin version must contain 1-64 bytes");
        }
        semver::Version::parse(&self.version)
            .context("plugin version must use semantic versioning")?;
        if self.name.trim().is_empty() || self.name.len() > 256 {
            bail!("plugin name must contain 1-256 bytes");
        }
        if self.description.trim().is_empty() || self.description.len() > 4096 {
            bail!("plugin description must contain 1-4096 bytes");
        }
        validate_relative_file(&self.entry)?;
        if !self.entry.ends_with(".lua") {
            bail!("plugin entry must be a Lua source file");
        }
        self.capabilities.validate()?;
        self.limits.validate()
    }
}

impl ExecutionLimits {
    /// 【插件】【资源校验】拒绝插件扩大到宿主硬上限之外的限制值。
    /// @returns 各项限制均在允许范围内时成功
    pub fn validate(&self) -> Result<()> {
        if !(1024 * 1024..=64 * 1024 * 1024).contains(&self.memory_bytes)
            || !(1_000..=20_000_000).contains(&self.instructions)
            || !(100..=900_000).contains(&self.timeout_ms)
            || !(1..=120_000).contains(&self.http_timeout_ms)
            || !(1024..=4 * 1024 * 1024).contains(&self.output_bytes)
        {
            bail!("plugin execution limits exceed supported bounds");
        }
        Ok(())
    }
}

/// 【插件】【名称校验】限制可进入工具名称和目录名的标识。
/// @param value 标识文本；max_len 为字节上限
/// @returns 标识以小写字母开头且仅包含小写字母、数字、短横线和下划线时成功
pub(crate) fn validate_identifier(value: &str, max_len: usize) -> Result<()> {
    if value.is_empty()
        || value.len() > max_len
        || !value.as_bytes()[0].is_ascii_lowercase()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_-".contains(&byte))
    {
        bail!("identifier must start with a-z and contain at most {max_len} letters, digits, '-' or '_'");
    }
    Ok(())
}

/// 【插件】【路径校验】验证跨平台相对源码路径。
/// @param value 使用正斜杠分隔的包内文件路径
/// @returns 不含绝对路径、父目录、反斜杠或盘符时成功
pub(crate) fn validate_relative_file(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 256
        || value.contains(['\\', ':', '<', '>', '"', '|', '?', '*'])
        || value.chars().any(char::is_control)
        || value.split('/').any(|part| {
            part.is_empty()
                || matches!(part, "." | "..")
                || part.ends_with(['.', ' '])
                || windows_device_name(part)
        })
    {
        bail!("plugin file must be a normalized relative path");
    }
    Ok(())
}

/// 【插件】【跨平台名称】识别 Windows 设备保留名，避免有效包在另一平台无法安装。
/// @param component 单个路径组成部分，可带扩展名
/// @returns 是否属于设备保留名
fn windows_device_name(component: &str) -> bool {
    let name = component
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        name.as_str(),
        "con" | "prn" | "aux" | "nul" | "conin$" | "conout$"
    ) || name
        .strip_prefix("com")
        .or_else(|| name.strip_prefix("lpt"))
        .is_some_and(|suffix| {
            matches!(
                suffix,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
}
