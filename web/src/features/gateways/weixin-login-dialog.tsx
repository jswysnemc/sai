import { useEffect, useRef, useState } from "react";
import { LoaderCircle } from "lucide-react";
import { Modal } from "../../shared/ui/dialog/modal";
import { api } from "../../api/client";
import { localizeApiMessage, toDisplayError } from "../../api/api-error";
import type { WeixinLoginAccount, WeixinLoginSnapshot } from "../../api/contracts";
import type { Locale } from "../i18n/locale";
import "./weixin-login-dialog.css";
import { useI18n } from "../i18n/use-i18n";

type WeixinLoginDialogProps = {
  open: boolean;
  baseUrl?: string;
  botType?: string;
  onClose: () => void;
  onConfirmed: (account: WeixinLoginAccount) => void;
};

/**
 * 将登录阶段和服务端消息映射为当前语言的界面提示文字。
 *
 * @param snapshot 微信登录快照
 * @param t 双语文本选择方法
 * @param locale 当前界面语言
 * @returns 本地化登录提示
 */
function phaseLabel(
  snapshot: WeixinLoginSnapshot | null,
  t: (en: string, zh: string) => string,
  locale: Locale
): string {
  if (!snapshot) return t("Fetching QR code", "正在获取二维码");
  if (snapshot.message) return localizeApiMessage(snapshot.message, locale);
  switch (snapshot.phase) {
    case "waiting":
      return t("Scan the QR code with Weixin on your phone", "请使用手机微信扫描二维码");
    case "scanned":
      return t("QR code scanned; confirm on your phone", "已扫码，等待手机确认");
    case "need_verify_code":
      return t("Verification code required", "需要输入验证码");
    case "confirmed":
      return t("Login successful", "登录成功");
    case "expired":
      return t("QR code expired", "二维码已过期");
    case "failed":
      return t("Login failed", "登录失败");
    default:
      return "";
  }
}

/**
 * 渲染微信扫码登录弹窗，负责发起登录、轮询状态、提交验证码与成功回填。
 *
 * @param props 弹窗开关、默认地址与回调
 * @returns 微信扫码登录弹窗
 */
export function WeixinLoginDialog({ open, baseUrl, botType, onClose, onConfirmed }: WeixinLoginDialogProps) {
  const { locale, t } = useI18n();
  const [snapshot, setSnapshot] = useState<WeixinLoginSnapshot | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [verifyCode, setVerifyCode] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const sessionRef = useRef<string | null>(null);
  const confirmedRef = useRef(false);

  // 打开时发起登录会话，关闭时清理状态
  useEffect(() => {
    if (!open) {
      sessionRef.current = null;
      confirmedRef.current = false;
      setSnapshot(null);
      setError(null);
      setVerifyCode("");
      return;
    }
    let cancelled = false;
    setError(null);
    setSnapshot(null);
    api.gateways.weixinLogin
      .start(baseUrl, botType)
      .then((result) => {
        if (cancelled) return;
        sessionRef.current = result.session_id;
        setSnapshot(result);
      })
      .catch((err: unknown) => {
        if (!cancelled) setError(toDisplayError(err, "Failed to start Weixin login", "启动微信登录失败"));
      });
    return () => {
      cancelled = true;
    };
  }, [open, baseUrl, botType]);

  // 轮询登录状态直至终态
  useEffect(() => {
    if (!open || !snapshot || confirmedRef.current) return;
    if (snapshot.phase === "confirmed" || snapshot.phase === "expired" || snapshot.phase === "failed") {
      if (snapshot.phase === "confirmed" && snapshot.account && !confirmedRef.current) {
        confirmedRef.current = true;
        onConfirmed(snapshot.account);
      }
      return;
    }
    const timer = window.setTimeout(async () => {
      const sessionId = sessionRef.current;
      if (!sessionId) return;
      try {
        const next = await api.gateways.weixinLogin.status(sessionId);
        setSnapshot(next);
      } catch (err) {
        setError(toDisplayError(err, "Failed to refresh Weixin login status", "刷新微信登录状态失败"));
      }
    }, 2000);
    return () => window.clearTimeout(timer);
  }, [open, snapshot, onConfirmed]);

  /** 提交验证码。 */
  const handleVerify = async () => {
    const sessionId = sessionRef.current;
    if (!sessionId || !verifyCode.trim()) return;
    setSubmitting(true);
    setError(null);
    try {
      const next = await api.gateways.weixinLogin.verify(sessionId, verifyCode.trim());
      setSnapshot(next);
      setVerifyCode("");
    } catch (err) {
      setError(toDisplayError(err, "Failed to submit verification code", "提交验证码失败"));
    } finally {
      setSubmitting(false);
    }
  };

  const needVerify = snapshot?.phase === "need_verify_code";
  const confirmed = snapshot?.phase === "confirmed";

  return (
    <Modal open={open} title={t("Weixin QR-code login", "微信扫码登录")} description={t("Scan the QR code with Weixin on your phone. Credentials are saved and filled into configuration automatically.", "使用手机微信扫描二维码完成登录，凭证将自动保存并回填配置。")} size="small" onClose={onClose}>
      <div className="weixin-login">
        <div className="weixin-login-qr">
          {snapshot?.qrcode_svg ? (
            <div className="weixin-login-qr-image" dangerouslySetInnerHTML={{ __html: snapshot.qrcode_svg }} />
          ) : (
            <div className="weixin-login-qr-placeholder"><LoaderCircle size={22} className="spin" /></div>
          )}
        </div>
        <p className={confirmed ? "weixin-login-status confirmed" : "weixin-login-status"}>{phaseLabel(snapshot, t, locale)}</p>
        {needVerify && (
          <div className="weixin-login-verify">
            <input
              value={verifyCode}
              onChange={(event) => setVerifyCode(event.target.value)}
              placeholder={t("Enter verification code", "输入验证码")}
              spellCheck={false}
              onKeyDown={(event) => { if (event.key === "Enter") void handleVerify(); }}
            />
            <button type="button" onClick={() => void handleVerify()} disabled={submitting || !verifyCode.trim()}>
              {submitting ? <LoaderCircle size={14} className="spin" /> : t("Submit", "提交")}
            </button>
          </div>
        )}
        {confirmed && snapshot?.account && (
          <dl className="weixin-login-account">
            <div><dt>{t("Account", "账号")}</dt><dd>{snapshot.account.account_id}</dd></div>
            <div><dt>{t("API address", "API 地址")}</dt><dd>{snapshot.account.base_url}</dd></div>
          </dl>
        )}
        {error && <div className="weixin-login-error">{error.message}</div>}
      </div>
    </Modal>
  );
}
