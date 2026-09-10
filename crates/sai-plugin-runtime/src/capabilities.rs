use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use url::Url;

mod binary;
mod system;
pub use binary::BinaryCapabilities;
pub use system::{ProcessArgument, ProcessParameter, ProcessTemplate, SystemCapabilities};

/// 【插件】【能力声明】来源授权与只读 POST 查询端点分别声明和授予。
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    #[serde(default)]
    pub http: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub http_read_only_post: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub model: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub notifications: bool,
    #[serde(default, skip_serializing_if = "BinaryCapabilities::is_empty")]
    pub binary: BinaryCapabilities,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub tools: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "SystemCapabilities::is_empty")]
    pub system: SystemCapabilities,
}

impl Capabilities {
    /// 【插件】【能力校验】验证精确来源和不含查询参数的规范端点地址。
    /// @returns 声明合法且没有超过数量限制时成功
    pub fn validate(&self) -> Result<()> {
        self.system.validate()?;
        self.binary.validate()?;
        if self.http.len() > 32 || self.http_read_only_post.len() > 32 {
            bail!("a plugin may declare at most 32 HTTP origins and 32 read-only POST endpoints");
        }
        for origin in &self.http {
            let url = parse_http_url(origin).context("invalid plugin HTTP origin")?;
            if url.query().is_some()
                || url.fragment().is_some()
                || url.path() != "/"
                || url.origin().ascii_serialization() != *origin
            {
                bail!("HTTP capability must be an exact origin");
            }
        }
        for endpoint in &self.http_read_only_post {
            let url = parse_http_url(endpoint).context("invalid read-only POST endpoint")?;
            if url.query().is_some() || url.fragment().is_some() || url.as_str() != endpoint {
                bail!("read-only POST endpoint must be a normalized URL without query or fragment");
            }
            if !self.http.contains(&url.origin().ascii_serialization()) {
                bail!("read-only POST endpoint must belong to a declared HTTP origin");
            }
        }
        if self.tools.len() > 128 {
            bail!("a plugin may declare at most 128 host tools");
        }
        for name in &self.tools {
            if name.is_empty()
                || name.len() > 64
                || !name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
            {
                bail!("tool capability must be an exact tool name of at most 64 bytes");
            }
        }
        Ok(())
    }

    /// 【插件】【授权交集】取声明和用户授权共有的来源与查询端点。
    /// @param granted 用户授予的能力集合
    /// @returns 不会增加来源或只读 POST 权限的有效集合
    pub fn intersection(&self, granted: &Self) -> Self {
        Self {
            http: self.http.intersection(&granted.http).cloned().collect(),
            http_read_only_post: self
                .http_read_only_post
                .intersection(&granted.http_read_only_post)
                .cloned()
                .collect(),
            model: self.model && granted.model,
            notifications: self.notifications && granted.notifications,
            binary: self.binary.intersection(&granted.binary),
            tools: self.tools.intersection(&granted.tools).cloned().collect(),
            system: self.system.intersection(&granted.system),
        }
    }

    /// 【插件】【授权范围】检查显式授予的每项能力是否已由清单声明。
    /// @param declared 清单中的能力集合
    /// @returns 当前集合是否完全包含于声明
    pub fn is_subset(&self, declared: &Self) -> bool {
        self.http.is_subset(&declared.http)
            && self
                .http_read_only_post
                .is_subset(&declared.http_read_only_post)
            && (!self.model || declared.model)
            && (!self.notifications || declared.notifications)
            && self.binary.is_subset(&declared.binary)
            && self.tools.is_subset(&declared.tools)
            && self.system.is_subset(&declared.system)
    }

    /// 【插件】【来源授权】检查完整 URL 是否属于有效来源，拒绝 URL 内嵌凭据。
    /// @param url 待访问地址
    /// @returns 解析后的已授权地址，错误不包含查询参数
    pub fn authorize_url(&self, url: &str) -> Result<Url> {
        let url = parse_http_url(url)?;
        if !self.http.contains(&url.origin().ascii_serialization()) {
            bail!("plugin HTTP origin is not allowed");
        }
        Ok(url)
    }

    /// 【插件】【请求授权】只读 POST 必须精确匹配端点，其余写入方法必须取得调用授权。
    /// @param method 大写 HTTP 方法；url 为目标地址；allow_writes 来自宿主调用上下文
    /// @returns 已授权的解析地址，初始化请求和每次重定向共用此检查
    pub fn authorize_request(&self, method: &str, url: &str, allow_writes: bool) -> Result<Url> {
        let url = self.authorize_url(url)?;
        if !matches!(method, "GET" | "HEAD" | "POST" | "PUT" | "PATCH" | "DELETE") {
            bail!("unsupported plugin HTTP method");
        }
        if allow_writes || matches!(method, "GET" | "HEAD") {
            return Ok(url);
        }
        let mut endpoint = url.clone();
        endpoint.set_query(None);
        endpoint.set_fragment(None);
        if method == "POST" && self.http_read_only_post.contains(endpoint.as_str()) {
            return Ok(url);
        }
        bail!("read-only plugin callback cannot make a writing HTTP request")
    }
}

/// 【插件】【URL 校验】只允许不含内嵌凭据且具有主机的 HTTP(S) 地址。
/// @param value 原始地址
/// @returns 解析地址，错误信息不重复输入中的秘密值
fn parse_http_url(value: &str) -> Result<Url> {
    if value.len() > 8192 {
        bail!("plugin HTTP URL exceeds size limit");
    }
    let url = Url::parse(value).context("invalid plugin HTTP URL")?;
    if url.as_str().len() > 8192 {
        bail!("plugin HTTP URL exceeds size limit");
    }
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        bail!("plugin HTTP URLs must use HTTP(S) and must not contain credentials");
    }
    Ok(url)
}
