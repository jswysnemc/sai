use super::{AppConfig, ModelEndpointConfig, ModelEndpointKind};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

/// TypeSafe 官方 systemone 评估地址。
pub const JEV_OFFICIAL_ENDPOINT: &str = "https://api.typesafe.ai/v1/systemone";
/// 未指定模型时使用的 Jev 模型。
pub const JEV_DEFAULT_MODEL: &str = "jev-latest";
/// 接入未提供密钥时依次读取的环境变量。
pub const JEV_KEY_ENV_NAMES: [&str; 2] = ["TYPESAFE_API_KEY", "TYPESAFE_KEY"];

/// 内置 Jev 功能配置：工具与 skills 暴露决策、权限自动审核共用同一接入。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct JevConfig {
    /// 引用 `model_endpoints` 中类型为 jev 的接入；留空时取第一条，仍没有时使用官方地址
    pub endpoint_id: String,
    /// 工具与 skills 暴露决策
    pub routing: JevRoutingConfig,
    /// 权限自动审核
    pub audit: JevAuditConfig,
}

/// 基于 Jev 的工具与 skills 暴露决策。
///
/// 开启后非基础工具一律延迟暴露：每轮请求前由 Jev 预选，
/// 模型也可通过 `request_capability` 向 Jev 追加申请。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct JevRoutingConfig {
    /// 是否开启
    pub enabled: bool,
    /// 候选被选中所需的最低 Noul 概率
    pub threshold: f64,
    /// 单次决策最多暴露的工具数
    pub max_tools: usize,
    /// 单次决策最多暴露的 skill 数
    pub max_skills: usize,
    /// 单次请求超时秒数
    pub timeout_seconds: u64,
    /// 作为判断依据的近期对话最大字符数
    pub context_chars: usize,
    /// 是否由 Jev 判断 `<jev>` 标签提示词片段；关闭时标签原文随静态提示发送
    pub prompt_segments: bool,
}

impl Default for JevRoutingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold: 0.5,
            max_tools: 6,
            max_skills: 3,
            timeout_seconds: 20,
            context_chars: 2_000,
            prompt_segments: true,
        }
    }
}

/// 基于 Jev Choice 的权限自动审核；只在自动审核权限模式下生效。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct JevAuditConfig {
    /// 是否开启；开启后优先于审核插件与聊天模型审核
    pub enabled: bool,
    /// 自动提交所需的最低选项概率
    pub minimum_probability: f64,
    /// 自动提交所需的最低置信度
    pub minimum_confidence: f64,
    /// 单次请求超时秒数
    pub timeout_seconds: u64,
}

impl Default for JevAuditConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            minimum_probability: 0.9,
            minimum_confidence: 0.8,
            timeout_seconds: 10,
        }
    }
}

/// Jev 接入来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JevConnectionSource {
    /// 设置中的 JEV 接入
    Endpoint,
    /// TypeSafe 官方地址与环境变量密钥
    Official,
}

/// 可展示的接入信息，不含密钥。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JevConnectionInfo {
    pub source: JevConnectionSource,
    pub endpoint_id: Option<String>,
    pub name: String,
    pub endpoint: String,
    pub model: String,
}

/// 可直接请求的接入。
#[derive(Debug, Clone)]
pub struct JevConnection {
    pub info: JevConnectionInfo,
    pub api_key: String,
}

impl JevConfig {
    /// 校验取值范围与接入引用。
    ///
    /// 参数:
    /// - `endpoints`: 当前全部专用模型接入
    ///
    /// 返回:
    /// - 校验结果
    pub fn validate(&self, endpoints: &[ModelEndpointConfig]) -> Result<()> {
        let id = self.endpoint_id.trim();
        if !id.is_empty() && find_endpoint(endpoints, id).is_none() {
            bail!("jev.endpoint_id {id} does not match a jev model endpoint");
        }
        let routing = &self.routing;
        if !(0.0..=1.0).contains(&routing.threshold) {
            bail!("jev.routing.threshold must be between 0 and 1");
        }
        if !(1..=60).contains(&routing.timeout_seconds) {
            bail!("jev.routing.timeout_seconds must be between 1 and 60");
        }
        let audit = &self.audit;
        for (name, value) in [
            ("minimum_probability", audit.minimum_probability),
            ("minimum_confidence", audit.minimum_confidence),
        ] {
            if !(0.5..=1.0).contains(&value) {
                bail!("jev.audit.{name} must be between 0.5 and 1");
            }
        }
        if !(1..=30).contains(&audit.timeout_seconds) {
            bail!("jev.audit.timeout_seconds must be between 1 and 30");
        }
        Ok(())
    }
}

impl AppConfig {
    /// 【Jev路由】【记忆注入】无参数；返回记忆是否交由当前有效路由按需暴露。
    pub fn jev_memory_injection_active(&self) -> bool {
        self.jev_routing_active()
            && self.memory_config().enabled
            && self.memory_config().jev_injection
    }

    /// 【Jev路由】【标签片段】无参数；返回 `<jev>` 标签片段是否交由 Jev 按需加载。
    pub fn jev_prompt_segments_active(&self) -> bool {
        self.jev_routing_active() && self.jev.routing.prompt_segments
    }

    /// 【Jev路由】【记忆注入】当前生效的记忆配置中是否开启按需注入。
    ///
    /// 返回:
    /// - 生效配置中的 `jev_injection`
    pub fn jev_memory_injection_enabled(&self) -> bool {
        self.memory_config().jev_injection
    }

    /// 【Jev路由】【记忆注入】写入按需注入开关。
    ///
    /// 顶层 `memory` 非默认时优先生效，两处都写，避免界面改了但运行时读另一处。
    ///
    /// 参数:
    /// - `enabled`: 是否开启
    ///
    /// 返回:
    /// - 无
    pub fn set_jev_memory_injection(&mut self, enabled: bool) {
        if self.memory != super::MemoryConfig::default() {
            self.memory.jev_injection = enabled;
        }
        self.plugins.memory.jev_injection = enabled;
    }

    /// 判断 Jev 暴露决策在当前会话是否生效。
    ///
    /// DeepSeek Anchored Standard 自带工具目录控制，两者同时开启时以锚定模式为准。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 开关打开且未启用锚定模式时返回 true
    pub fn jev_routing_active(&self) -> bool {
        self.jev.routing.enabled && !self.active_deepseek_anchor_enabled().unwrap_or(false)
    }

    /// 返回当前生效的 JEV 接入；未配置时为空，表示使用官方地址。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 引用的接入；`endpoint_id` 指向不存在的接入时报错
    pub fn jev_endpoint(&self) -> Result<Option<&ModelEndpointConfig>> {
        let id = self.jev.endpoint_id.trim();
        if id.is_empty() {
            return Ok(self
                .model_endpoints
                .iter()
                .find(|item| item.kind == ModelEndpointKind::Jev));
        }
        find_endpoint(&self.model_endpoints, id)
            .map(Some)
            .with_context(|| format!("jev model endpoint {id} not found"))
    }

    /// 解析可直接请求的 Jev 接入与密钥。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 接入与密钥；接入不存在或密钥缺失时报错
    pub fn jev_connection(&self) -> Result<JevConnection> {
        jev_connection_for(self.jev_endpoint()?)
    }
}

/// 按接入构造可请求的连接；接入为空时使用官方地址。
///
/// 参数:
/// - `endpoint`: JEV 接入，可为浏览器提交的草稿
///
/// 返回:
/// - 接入与密钥；密钥缺失时报错且不包含敏感值
pub fn jev_connection_for(endpoint: Option<&ModelEndpointConfig>) -> Result<JevConnection> {
    let info = jev_connection_info_for(endpoint);
    // 1. 接入自带密钥优先；环境变量引用缺失时直接报错，避免静默换用其他凭据
    let own_key = match endpoint {
        Some(item) => item.resolved_api_key()?.trim().to_string(),
        None => String::new(),
    };
    // 2. 未填写密钥时回退 TypeSafe 环境变量
    let api_key = if own_key.is_empty() {
        JEV_KEY_ENV_NAMES
            .iter()
            .find_map(|name| {
                std::env::var(name)
                    .ok()
                    .filter(|value| !value.trim().is_empty())
            })
            .map(|value| value.trim().to_string())
            .with_context(|| {
                format!(
                    "Jev API key is not configured; set {} or add a key to the JEV connection",
                    JEV_KEY_ENV_NAMES.join(" or ")
                )
            })?
    } else {
        own_key
    };
    Ok(JevConnection { info, api_key })
}

/// 生成不含密钥的接入信息。
///
/// 参数:
/// - `endpoint`: JEV 接入；为空时描述官方地址
///
/// 返回:
/// - 可展示的接入信息
pub fn jev_connection_info_for(endpoint: Option<&ModelEndpointConfig>) -> JevConnectionInfo {
    match endpoint {
        Some(item) => JevConnectionInfo {
            source: JevConnectionSource::Endpoint,
            endpoint_id: Some(item.id.clone()),
            name: item.name.clone(),
            endpoint: systemone_url(&item.endpoint),
            model: non_empty_or(&item.model, JEV_DEFAULT_MODEL),
        },
        None => JevConnectionInfo {
            source: JevConnectionSource::Official,
            endpoint_id: None,
            name: "TypeSafe".to_string(),
            endpoint: JEV_OFFICIAL_ENDPOINT.to_string(),
            model: JEV_DEFAULT_MODEL.to_string(),
        },
    }
}

/// 把 TypeSafe 根地址补全为 systemone 地址；其他完整地址保持原样。
///
/// 参数:
/// - `raw`: 接入中填写的地址
///
/// 返回:
/// - 实际请求地址
fn systemone_url(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.ends_with("/v1") {
        format!("{trimmed}/systemone")
    } else {
        trimmed.to_string()
    }
}

/// 按 id 查找 JEV 接入。
fn find_endpoint<'a>(
    endpoints: &'a [ModelEndpointConfig],
    id: &str,
) -> Option<&'a ModelEndpointConfig> {
    endpoints
        .iter()
        .find(|item| item.kind == ModelEndpointKind::Jev && item.id == id)
}

/// 去除首尾空白后为空时使用缺省值。
fn non_empty_or(value: &str, fallback: &str) -> String {
    let value = value.trim();
    if value.is_empty() { fallback } else { value }.to_string()
}

#[cfg(test)]
#[path = "jev_tests.rs"]
mod tests;
