use serde::{Deserialize, Serialize};

/// 供应商的单个 API Key，带稳定标识与可选备注。
///
/// 稳定 `id` 是脱敏与哨兵回填的对齐键：前端编辑后提交时，
/// 服务端按 `id` 匹配旧值，避免删除或重排后串用密钥。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderApiKey {
    /// 稳定标识，由前端生成，用于脱敏回填对齐
    pub id: String,
    /// 密钥值；支持 `$env:NAME` 引用
    pub api_key: String,
    /// 用户可读备注，如「主号」「备用」
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
}
