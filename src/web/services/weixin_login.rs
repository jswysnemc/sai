use crate::gateways::weixin_bot::login::{
    default_base_url, default_bot_type, fetch_qrcode, poll_qrcode_status, save_weixin_account,
    update_weixin_gateway_config, SavedWeixinAccount,
};
use crate::paths::SaiPaths;
use anyhow::Result;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::Mutex as AsyncMutex;

const LOGIN_TIMEOUT: Duration = Duration::from_secs(180);

/// 微信 web 登录阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WeixinLoginPhase {
    Waiting,
    Scanned,
    NeedVerifyCode,
    Confirmed,
    Expired,
    Failed,
}

impl WeixinLoginPhase {
    /// 判断是否为终态。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 处于终态时返回 true
    fn is_terminal(self) -> bool {
        matches!(self, Self::Confirmed | Self::Expired | Self::Failed)
    }
}

/// 微信 web 登录成功回填数据。
#[derive(Debug, Clone, Serialize)]
pub(crate) struct WeixinLoginAccount {
    pub(crate) account_id: String,
    pub(crate) base_url: String,
    pub(crate) cdn_base_url: String,
    pub(crate) user_id: Option<String>,
}

/// 微信 web 登录会话状态快照。
#[derive(Debug, Clone, Serialize)]
pub(crate) struct WeixinLoginSnapshot {
    pub(crate) session_id: String,
    pub(crate) phase: WeixinLoginPhase,
    pub(crate) qrcode_content: String,
    pub(crate) qrcode_svg: String,
    pub(crate) message: Option<String>,
    pub(crate) account: Option<WeixinLoginAccount>,
}

/// 单个微信登录会话的内部状态。
#[derive(Debug, Clone)]
struct LoginSession {
    phase: WeixinLoginPhase,
    qrcode_content: String,
    qrcode_svg: String,
    message: Option<String>,
    account: Option<WeixinLoginAccount>,
    verify_code: Option<String>,
    base_url: String,
    qrcode: String,
}

/// 微信 web 登录会话管理器。
#[derive(Clone)]
pub(crate) struct WeixinLoginManager {
    paths: SaiPaths,
    sessions: Arc<Mutex<HashMap<String, LoginSession>>>,
    poll_lock: Arc<AsyncMutex<()>>,
}

impl WeixinLoginManager {
    /// 创建微信登录会话管理器。
    ///
    /// 参数:
    /// - `paths`: Sai 路径
    ///
    /// 返回:
    /// - 微信登录会话管理器
    pub(crate) fn new(paths: &SaiPaths) -> Self {
        Self {
            paths: paths.clone(),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            poll_lock: Arc::new(AsyncMutex::new(())),
        }
    }

    /// 发起一次微信扫码登录，返回二维码快照。
    ///
    /// 参数:
    /// - `base_url`: 可选自定义 iLink API 地址
    /// - `bot_type`: 可选自定义 bot_type
    ///
    /// 返回:
    /// - 新登录会话快照
    pub(crate) async fn start(
        &self,
        base_url: Option<String>,
        bot_type: Option<String>,
    ) -> Result<WeixinLoginSnapshot> {
        let client = reqwest::Client::new();
        let base_url = base_url
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| default_base_url().to_string())
            .trim_end_matches('/')
            .to_string();
        let bot_type = bot_type
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| default_bot_type().to_string());
        let qr = fetch_qrcode(&client, &base_url, &bot_type).await?;
        let session_id = format!("wxlogin_{}", uuid::Uuid::new_v4().simple());
        let qrcode_svg = render_qrcode_svg(&qr.qrcode_img_content);
        let session = LoginSession {
            phase: WeixinLoginPhase::Waiting,
            qrcode_content: qr.qrcode_img_content.clone(),
            qrcode_svg,
            message: None,
            account: None,
            verify_code: None,
            base_url,
            qrcode: qr.qrcode,
        };
        let snapshot = snapshot_from(&session_id, &session);
        self.sessions
            .lock()
            .unwrap()
            .insert(session_id.clone(), session);
        self.spawn_poll_task(session_id);
        Ok(snapshot)
    }

    /// 读取指定登录会话的当前状态。
    ///
    /// 参数:
    /// - `session_id`: 登录会话标识
    ///
    /// 返回:
    /// - 登录会话快照，会话不存在时返回空
    pub(crate) fn status(&self, session_id: &str) -> Option<WeixinLoginSnapshot> {
        let sessions = self.sessions.lock().unwrap();
        sessions
            .get(session_id)
            .map(|session| snapshot_from(session_id, session))
    }

    /// 向指定登录会话提交验证码。
    ///
    /// 参数:
    /// - `session_id`: 登录会话标识
    /// - `verify_code`: 用户输入验证码
    ///
    /// 返回:
    /// - 更新后的登录会话快照，会话不存在时返回空
    pub(crate) fn submit_verify_code(
        &self,
        session_id: &str,
        verify_code: &str,
    ) -> Option<WeixinLoginSnapshot> {
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions.get_mut(session_id)?;
        session.verify_code = Some(verify_code.trim().to_string());
        session.phase = WeixinLoginPhase::Waiting;
        session.message = Some("verification code submitted; awaiting confirmation".to_string());
        Some(snapshot_from(session_id, session))
    }

    /// 后台轮询指定登录会话的二维码状态。
    ///
    /// 参数:
    /// - `session_id`: 登录会话标识
    ///
    /// 返回:
    /// - 无
    fn spawn_poll_task(&self, session_id: String) {
        let manager = self.clone();
        tokio::spawn(async move {
            let _guard = manager.poll_lock.lock().await;
            let client = reqwest::Client::new();
            let deadline = Instant::now() + LOGIN_TIMEOUT;
            loop {
                if Instant::now() >= deadline {
                    manager.mark_expired(&session_id);
                    break;
                }
                let Some((base_url, qrcode, verify_code)) = manager.poll_inputs(&session_id) else {
                    break;
                };
                let status =
                    poll_qrcode_status(&client, &base_url, &qrcode, verify_code.as_deref()).await;
                let done = match status {
                    Ok(status) => manager.apply_status(&session_id, status),
                    Err(_) => false,
                };
                if done {
                    break;
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        });
    }

    /// 读取轮询所需的会话输入。
    ///
    /// 参数:
    /// - `session_id`: 登录会话标识
    ///
    /// 返回:
    /// - 基础地址、二维码 ID 与可选验证码，会话已终态时返回空
    fn poll_inputs(&self, session_id: &str) -> Option<(String, String, Option<String>)> {
        let sessions = self.sessions.lock().unwrap();
        let session = sessions.get(session_id)?;
        if session.phase.is_terminal() {
            return None;
        }
        Some((
            session.base_url.clone(),
            session.qrcode.clone(),
            session.verify_code.clone(),
        ))
    }

    /// 应用一次二维码状态轮询结果。
    ///
    /// 参数:
    /// - `session_id`: 登录会话标识
    /// - `status`: 二维码状态响应
    ///
    /// 返回:
    /// - 是否达到终态需要停止轮询
    fn apply_status(
        &self,
        session_id: &str,
        status: crate::gateways::weixin_bot::login::QrStatusResponse,
    ) -> bool {
        let mut sessions = self.sessions.lock().unwrap();
        let Some(session) = sessions.get_mut(session_id) else {
            return true;
        };
        match status.status.as_str() {
            "wait" => false,
            "scaned" => {
                session.phase = WeixinLoginPhase::Scanned;
                session.message = Some("QR code scanned; confirm on your phone".to_string());
                false
            }
            "scaned_but_redirect" => {
                if let Some(host) = status
                    .redirect_host
                    .filter(|value| value.starts_with("http"))
                {
                    session.base_url = host.trim_end_matches('/').to_string();
                }
                session.phase = WeixinLoginPhase::Scanned;
                false
            }
            "need_verifycode" => {
                // 已提交验证码时保持等待，避免覆盖用户输入
                if session.verify_code.is_none() {
                    session.phase = WeixinLoginPhase::NeedVerifyCode;
                    session.message = Some("verification code required".to_string());
                }
                false
            }
            "verify_code_blocked" => {
                session.verify_code = None;
                session.phase = WeixinLoginPhase::NeedVerifyCode;
                session.message = Some("verification code rejected; enter it again".to_string());
                false
            }
            "expired" => {
                session.phase = WeixinLoginPhase::Expired;
                session.message = Some("QR code expired; request a new one".to_string());
                true
            }
            "binded_redirect" => {
                session.phase = WeixinLoginPhase::Failed;
                session.message = Some(
                    "this Weixin account is already linked, but no new credentials were returned; start the service directly if the account is saved locally".to_string(),
                );
                true
            }
            "confirmed" => {
                self.finish_confirmed(session, status);
                true
            }
            other => {
                session.phase = WeixinLoginPhase::Failed;
                session.message = Some(format!("unknown login status: {other}"));
                true
            }
        }
    }

    /// 处理登录确认，保存账号并回填配置。
    ///
    /// 参数:
    /// - `session`: 登录会话
    /// - `status`: 确认状态响应
    ///
    /// 返回:
    /// - 无
    fn finish_confirmed(
        &self,
        session: &mut LoginSession,
        status: crate::gateways::weixin_bot::login::QrStatusResponse,
    ) {
        let token = status.bot_token.filter(|value| !value.trim().is_empty());
        let account_id = status.ilink_bot_id.filter(|value| !value.trim().is_empty());
        let (Some(token), Some(account_id)) = (token, account_id) else {
            session.phase = WeixinLoginPhase::Failed;
            session.message = Some("login succeeded but credentials are missing".to_string());
            return;
        };
        let saved = SavedWeixinAccount {
            account_id: account_id.clone(),
            token,
            base_url: status
                .baseurl
                .filter(|value| value.starts_with("http"))
                .unwrap_or_else(|| session.base_url.clone()),
            cdn_base_url: crate::gateways::weixin_bot::client::default_cdn_base_url().to_string(),
            user_id: status.ilink_user_id.clone(),
            saved_at: chrono::Utc::now().to_rfc3339(),
        };
        if let Err(err) = self.persist_account(&saved) {
            session.phase = WeixinLoginPhase::Failed;
            session.message = Some(format!("failed to save credentials: {err:#}"));
            return;
        }
        session.phase = WeixinLoginPhase::Confirmed;
        session.message = Some("login successful".to_string());
        session.account = Some(WeixinLoginAccount {
            account_id: saved.account_id.clone(),
            base_url: saved.base_url.clone(),
            cdn_base_url: saved.cdn_base_url.clone(),
            user_id: saved.user_id.clone(),
        });
    }

    /// 保存微信账号并回填网关配置。
    ///
    /// 参数:
    /// - `saved`: 待保存账号
    ///
    /// 返回:
    /// - 保存是否成功
    fn persist_account(&self, saved: &SavedWeixinAccount) -> Result<()> {
        save_weixin_account(&self.paths, saved)?;
        update_weixin_gateway_config(&self.paths, saved)?;
        Ok(())
    }

    /// 将会话标记为超时过期。
    ///
    /// 参数:
    /// - `session_id`: 登录会话标识
    ///
    /// 返回:
    /// - 无
    fn mark_expired(&self, session_id: &str) {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.get_mut(session_id) {
            if !session.phase.is_terminal() {
                session.phase = WeixinLoginPhase::Expired;
                session.message = Some("login timed out; request a new QR code".to_string());
            }
        }
    }
}

/// 从内部会话状态构造对外快照。
///
/// 参数:
/// - `session_id`: 登录会话标识
/// - `session`: 内部会话状态
///
/// 返回:
/// - 登录会话快照
fn snapshot_from(session_id: &str, session: &LoginSession) -> WeixinLoginSnapshot {
    WeixinLoginSnapshot {
        session_id: session_id.to_string(),
        phase: session.phase,
        qrcode_content: session.qrcode_content.clone(),
        qrcode_svg: session.qrcode_svg.clone(),
        message: session.message.clone(),
        account: session.account.clone(),
    }
}

/// 将二维码内容渲染为 SVG 字符串，供前端直接展示。
///
/// 参数:
/// - `content`: 二维码内容
///
/// 返回:
/// - 二维码 SVG，渲染失败时返回空串
fn render_qrcode_svg(content: &str) -> String {
    use qrcode::render::svg;
    match qrcode::QrCode::new(content.as_bytes()) {
        Ok(code) => code
            .render::<svg::Color>()
            .min_dimensions(220, 220)
            .dark_color(svg::Color("#111111"))
            .light_color(svg::Color("#ffffff"))
            .build(),
        Err(_) => String::new(),
    }
}
