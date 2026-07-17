use super::client::default_cdn_base_url;
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use anyhow::{bail, Context, Result};
use base64::Engine;
use qrcode::render::unicode;
use qrcode::QrCode;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

const DEFAULT_BASE_URL: &str = "https://ilinkai.weixin.qq.com";
const DEFAULT_BOT_TYPE: &str = "3";
const ILINK_APP_ID: &str = "bot";
const ILINK_APP_CLIENT_VERSION: &str = "132102";
const QR_POLL_TIMEOUT: Duration = Duration::from_secs(40);

#[derive(Debug, Clone)]
pub(crate) struct WeixinLoginConfig {
    pub(crate) base_url: String,
    pub(crate) bot_type: String,
    pub(crate) timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SavedWeixinAccount {
    pub(crate) account_id: String,
    pub(crate) token: String,
    pub(crate) base_url: String,
    #[serde(default = "default_cdn_base_url_string")]
    pub(crate) cdn_base_url: String,
    pub(crate) user_id: Option<String>,
    pub(crate) saved_at: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct QrCodeResponse {
    pub(crate) qrcode: String,
    pub(crate) qrcode_img_content: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct QrStatusResponse {
    pub(crate) status: String,
    pub(crate) bot_token: Option<String>,
    pub(crate) ilink_bot_id: Option<String>,
    pub(crate) baseurl: Option<String>,
    pub(crate) ilink_user_id: Option<String>,
    pub(crate) redirect_host: Option<String>,
}

/// 返回微信登录默认基础地址。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 微信 iLink API 基础地址
pub(crate) fn default_base_url() -> &'static str {
    DEFAULT_BASE_URL
}

/// 返回微信登录默认 bot_type。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 默认 bot_type
pub(crate) fn default_bot_type() -> &'static str {
    DEFAULT_BOT_TYPE
}

/// 返回微信默认 CDN 基础地址字符串。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 微信 CDN 基础地址
fn default_cdn_base_url_string() -> String {
    default_cdn_base_url().to_string()
}

/// 执行微信二维码登录。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 登录配置
///
/// 返回:
/// - 登录是否成功
pub(crate) async fn run_weixin_login(paths: &SaiPaths, config: WeixinLoginConfig) -> Result<()> {
    let client = reqwest::Client::new();
    let mut base_url = config.base_url.trim_end_matches('/').to_string();
    let qr = fetch_qrcode(&client, &base_url, &config.bot_type).await?;
    println!("微信二维码登录链接:");
    println!("{}", qr.qrcode_img_content);
    println!();
    print_qrcode(&qr.qrcode_img_content)?;
    println!("请使用手机微信扫描二维码并确认登录。");
    let deadline = Instant::now() + Duration::from_secs(config.timeout_secs);
    let mut verify_code = None::<String>;
    let mut printed_scanned = false;
    loop {
        if Instant::now() >= deadline {
            bail!("微信二维码登录超时");
        }
        let status = poll_qrcode_status(&client, &base_url, &qr.qrcode, verify_code.as_deref())
            .await
            .unwrap_or_else(|err| QrStatusResponse {
                status: "wait".to_string(),
                bot_token: None,
                ilink_bot_id: None,
                baseurl: None,
                ilink_user_id: None,
                redirect_host: Some(format!("poll failed: {err}")),
            });
        match status.status.as_str() {
            "wait" => {}
            "scaned" => {
                if !printed_scanned {
                    println!("已扫码，等待手机确认。");
                    printed_scanned = true;
                }
            }
            "scaned_but_redirect" => {
                if let Some(host) = status
                    .redirect_host
                    .filter(|value| value.starts_with("http"))
                {
                    base_url = host.trim_end_matches('/').to_string();
                }
            }
            "need_verifycode" => {
                verify_code = Some(read_verify_code()?);
            }
            "verify_code_blocked" => {
                bail!("微信验证码被拒绝，请重新运行登录命令");
            }
            "expired" => {
                bail!("微信二维码已过期，请重新运行登录命令");
            }
            "binded_redirect" => {
                bail!("该微信账号已经绑定，但本次未返回新 token；如果本机已有保存账号，可直接运行 weixin-server");
            }
            "confirmed" => {
                let token = status
                    .bot_token
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| anyhow::anyhow!("微信登录成功但响应中没有 bot_token"))?;
                let account_id = status
                    .ilink_bot_id
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| anyhow::anyhow!("微信登录成功但响应中没有 ilink_bot_id"))?;
                let saved = SavedWeixinAccount {
                    account_id,
                    token,
                    base_url: status
                        .baseurl
                        .filter(|value| value.starts_with("http"))
                        .unwrap_or(base_url),
                    cdn_base_url: default_cdn_base_url().to_string(),
                    user_id: status.ilink_user_id,
                    saved_at: chrono::Utc::now().to_rfc3339(),
                };
                let path = save_weixin_account(paths, &saved)?;
                update_weixin_gateway_config(paths, &saved)?;
                println!("微信登录成功。");
                println!("账号: {}", saved.account_id);
                println!("凭证已保存: {}", path.display());
                println!(
                    "启动命令: sai gateway weixin-server --account {}",
                    saved.account_id
                );
                return Ok(());
            }
            other => {
                bail!("未知微信登录状态: {other}");
            }
        }
    }
}

/// 回写微信网关 TUI 配置。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `account`: 已保存微信账号
///
/// 返回:
/// - 回写是否成功
pub(crate) fn update_weixin_gateway_config(
    paths: &SaiPaths,
    account: &SavedWeixinAccount,
) -> Result<()> {
    AppConfig::init_files(paths)?;
    let mut config = AppConfig::load_or_default(paths)?;
    config.gateways.weixin.enabled = true;
    config.gateways.weixin.account = account.account_id.clone();
    config.gateways.weixin.base_url = account.base_url.clone();
    config.gateways.weixin.cdn_base_url = account.cdn_base_url.clone();
    config.gateways.weixin.token.clear();
    config.save(paths)
}

/// 读取已保存的微信账号。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `account_id`: 账号 ID；为空时读取最近登录账号
///
/// 返回:
/// - 已保存账号
pub(crate) fn load_weixin_account(
    paths: &SaiPaths,
    account_id: Option<&str>,
) -> Result<SavedWeixinAccount> {
    let account_id = match account_id.filter(|value| !value.trim().is_empty()) {
        Some(value) => value.to_string(),
        None => {
            let latest = weixin_state_dir(paths).join("latest.json");
            let raw = std::fs::read_to_string(&latest).with_context(|| {
                format!("failed to read latest Weixin account: {}", latest.display())
            })?;
            serde_json::from_str::<Value>(&raw)?
                .get("account_id")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| anyhow::anyhow!("latest Weixin account file has no account_id"))?
        }
    };
    let path = account_file(paths, &account_id);
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read Weixin account: {}", path.display()))?;
    serde_json::from_str::<SavedWeixinAccount>(&raw)
        .with_context(|| format!("invalid Weixin account file: {}", path.display()))
}

/// 请求微信登录二维码。
///
/// 参数:
/// - `client`: HTTP 客户端
/// - `base_url`: iLink API 基础地址
/// - `bot_type`: 微信 bot_type
///
/// 返回:
/// - 二维码响应
pub(crate) async fn fetch_qrcode(
    client: &reqwest::Client,
    base_url: &str,
    bot_type: &str,
) -> Result<QrCodeResponse> {
    let url = format!(
        "{}/ilink/bot/get_bot_qrcode?bot_type={}",
        base_url.trim_end_matches('/'),
        urlencoding::encode(bot_type)
    );
    let response = client
        .post(&url)
        .headers(login_headers()?)
        .json(&json!({ "local_token_list": [] }))
        .send()
        .await
        .with_context(|| "failed to request Weixin login QR code")?;
    parse_response(response, "Weixin get_bot_qrcode").await
}

/// 轮询微信二维码登录状态。
///
/// 参数:
/// - `client`: HTTP 客户端
/// - `base_url`: iLink API 基础地址
/// - `qrcode`: 二维码 ID
/// - `verify_code`: 可选验证码
///
/// 返回:
/// - 登录状态响应
pub(crate) async fn poll_qrcode_status(
    client: &reqwest::Client,
    base_url: &str,
    qrcode: &str,
    verify_code: Option<&str>,
) -> Result<QrStatusResponse> {
    let mut url = format!(
        "{}/ilink/bot/get_qrcode_status?qrcode={}",
        base_url.trim_end_matches('/'),
        urlencoding::encode(qrcode)
    );
    if let Some(code) = verify_code.filter(|value| !value.trim().is_empty()) {
        url.push_str("&verify_code=");
        url.push_str(&urlencoding::encode(code));
    }
    let response = client
        .get(&url)
        .headers(login_headers()?)
        .timeout(QR_POLL_TIMEOUT)
        .send()
        .await
        .with_context(|| "failed to poll Weixin QR status")?;
    parse_response(response, "Weixin get_qrcode_status").await
}

/// 解析 JSON HTTP 响应。
///
/// 参数:
/// - `response`: HTTP 响应
/// - `label`: API 标签
///
/// 返回:
/// - 解析后的 JSON 结构
async fn parse_response<T>(response: reqwest::Response, label: &str) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!("{label} returned HTTP {status}: {body}");
    }
    serde_json::from_str::<T>(&body).with_context(|| format!("invalid {label} response: {body}"))
}

/// 在终端打印二维码。
///
/// 参数:
/// - `content`: 二维码内容
///
/// 返回:
/// - 是否打印成功
fn print_qrcode(content: &str) -> Result<()> {
    let code = QrCode::new(content.as_bytes()).with_context(|| "failed to build QR code")?;
    let image = code
        .render::<unicode::Dense1x2>()
        .dark_color(unicode::Dense1x2::Dark)
        .light_color(unicode::Dense1x2::Light)
        .build();
    println!("{image}");
    Ok(())
}

/// 从终端读取微信验证码。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 验证码
fn read_verify_code() -> Result<String> {
    print!("需要验证码，请输入后回车: ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let code = input.trim().to_string();
    if code.is_empty() {
        bail!("验证码不能为空");
    }
    Ok(code)
}

/// 保存微信登录账号。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `account`: 账号数据
///
/// 返回:
/// - 保存文件路径
pub(crate) fn save_weixin_account(
    paths: &SaiPaths,
    account: &SavedWeixinAccount,
) -> Result<PathBuf> {
    let dir = weixin_accounts_dir(paths);
    std::fs::create_dir_all(&dir)?;
    let path = account_file(paths, &account.account_id);
    std::fs::write(&path, serde_json::to_vec_pretty(account)?)?;
    std::fs::write(
        weixin_state_dir(paths).join("latest.json"),
        serde_json::to_vec_pretty(&json!({ "account_id": account.account_id }))?,
    )?;
    Ok(path)
}

/// 返回微信账号状态目录。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - 微信状态目录
fn weixin_state_dir(paths: &SaiPaths) -> PathBuf {
    paths.state_dir.join("gateways").join("weixin")
}

/// 返回微信账号保存目录。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - 微信账号目录
fn weixin_accounts_dir(paths: &SaiPaths) -> PathBuf {
    weixin_state_dir(paths).join("accounts")
}

/// 返回微信账号保存文件路径。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `account_id`: 账号 ID
///
/// 返回:
/// - 账号文件路径
fn account_file(paths: &SaiPaths, account_id: &str) -> PathBuf {
    weixin_accounts_dir(paths).join(format!("{}.json", sanitize_account_id(account_id)))
}

/// 清理账号 ID 文件名。
///
/// 参数:
/// - `account_id`: 原始账号 ID
///
/// 返回:
/// - 安全文件名
fn sanitize_account_id(account_id: &str) -> String {
    account_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_' | '@') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

/// 构建微信登录请求头。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 请求头集合
fn login_headers() -> Result<reqwest::header::HeaderMap> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("Content-Type", "application/json".parse()?);
    headers.insert("X-WECHAT-UIN", random_wechat_uin().parse()?);
    headers.insert("iLink-App-Id", ILINK_APP_ID.parse()?);
    headers.insert("iLink-App-ClientVersion", ILINK_APP_CLIENT_VERSION.parse()?);
    Ok(headers)
}

/// 生成 X-WECHAT-UIN 请求头。
///
/// 参数:
/// - 无
///
/// 返回:
/// - base64 编码的随机 uint32 字符串
fn random_wechat_uin() -> String {
    let value = rand::thread_rng().next_u32().to_string();
    base64::engine::general_purpose::STANDARD.encode(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_account_id_for_file_name() {
        assert_eq!(sanitize_account_id("abc@im.bot"), "abc@im.bot");
        assert_eq!(sanitize_account_id("a/b:c"), "a_b_c");
    }
}
