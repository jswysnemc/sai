import { KeyRound, Loader2 } from "lucide-react";
import { useState, type FormEvent } from "react";
import { loginWithPassword } from "../../api/client";
import { SaiLogo } from "../../shared/ui/sai-logo";
import { useI18n } from "../i18n/use-i18n";
import "./password-login.css";

type PasswordLoginProps = {
  onAuthenticated: () => void;
};

/**
 * 渲染 Sai Web 的口令登录页。
 *
 * 服务端启用访问口令后，浏览器需先通过此页建立会话；
 * 启动令牌在该模式下不再单独放行，避免令牌泄露即可接管实例。
 *
 * @param props 登录成功回调
 * @returns 口令登录页
 */
export function PasswordLogin(props: PasswordLoginProps) {
  const { t } = useI18n();
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  /**
   * 提交口令并在通过后进入工作台。
   *
   * @param event 表单提交事件
   * @returns 无
   */
  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!password || busy) return;
    setBusy(true);
    setError("");
    try {
      await loginWithPassword(password);
      props.onAuthenticated();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
      setPassword("");
    } finally {
      setBusy(false);
    }
  };

  return (
    <main className="password-login">
      <form className="password-login-card" onSubmit={(event) => void submit(event)}>
        <SaiLogo />
        <p className="password-login-hint">
          {t("Enter the access password to continue.", "输入访问口令以继续。")}
        </p>
        <label className="password-login-field">
          <span>{t("Password", "访问口令")}</span>
          <input
            type="password"
            value={password}
            onChange={(event) => setPassword(event.target.value)}
            autoComplete="current-password"
            autoFocus
            disabled={busy}
          />
        </label>
        {error && <p className="password-login-error">{error}</p>}
        <button type="submit" disabled={busy || !password}>
          {busy ? <Loader2 size={14} className="password-login-spin" /> : <KeyRound size={14} />}
          {t("Sign in", "登录")}
        </button>
      </form>
    </main>
  );
}
