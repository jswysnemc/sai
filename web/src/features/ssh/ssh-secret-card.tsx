import { KeyRound, ShieldAlert } from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "../../api/client";
import { toDisplayError } from "../../api/api-error";
import type { SshSecretRequest } from "../../api/contracts";
import { Button } from "../../shared/ui/button/button";
import { useI18n } from "../i18n/use-i18n";
import "./ssh-secret-card.css";

type SshSecretCardProps = {
  request: SshSecretRequest;
  resolved?: boolean;
  active?: boolean;
};

/**
 * 在助手消息流内渲染 SSH 交互式安全输入卡片。
 *
 * 口令/密码通过 password 输入框采集（浏览器不回显），主机指纹与高危命令走确认按钮。
 * 秘密仅在提交请求体里一次性发往后端，绝不进入事件流、消息或模型上下文。
 *
 * @param props SSH 交互征询与展示状态
 * @returns 内嵌安全输入卡片
 */
export function SshSecretCard({ request, resolved = false, active = true }: SshSecretCardProps) {
  const { t } = useI18n();
  const isSecret =
    request.kind === "passphrase" || request.kind === "password" || request.kind === "sudo_password";
  const [secret, setSecret] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [done, setDone] = useState(resolved);
  const [error, setError] = useState<Error | null>(null);

  useEffect(() => {
    setDone(resolved);
  }, [resolved]);
  useEffect(() => {
    setSecret("");
    setSubmitting(false);
    setError(null);
  }, [request.id]);

  const submit = async (body: { secret?: string; confirmed?: boolean; cancelled?: boolean }) => {
    setSubmitting(true);
    setError(null);
    try {
      await api.sshSecrets.submit(request.id, body);
      setDone(true);
      setSecret("");
    } catch (cause) {
      setError(toDisplayError(cause, "Failed to submit the SSH input", "提交 SSH 输入失败"));
    } finally {
      setSubmitting(false);
    }
  };

  const interactive = !done && active;

  return (
    <div className={`ssh-secret-card${done ? " is-done" : ""}`}>
      <div className="ssh-secret-head">
        {isSecret ? <KeyRound size={14} /> : <ShieldAlert size={14} />}
        <span className="ssh-secret-title">{titleOf(request.kind, t)}</span>
        <span className="ssh-secret-host">{request.host_label}</span>
      </div>
      <div className="ssh-secret-prompt">{request.prompt}</div>
      {request.fingerprint ? (
        <div className="ssh-secret-fingerprint">
          <code>SHA256 {request.fingerprint}</code>
          {request.changed ? (
            <div className="ssh-secret-warning">
              {t(
                "This host key differs from the known_hosts record — possible man-in-the-middle.",
                "该主机指纹与 known_hosts 记录不一致，可能存在中间人风险。"
              )}
            </div>
          ) : null}
        </div>
      ) : null}
      {interactive && isSecret ? (
        <div className="ssh-secret-actions">
          <input
            className="ssh-secret-input"
            type="password"
            value={secret}
            autoFocus
            autoComplete="off"
            placeholder={t("Enter secret (never sent to the model)", "输入秘密（不会发送给模型）")}
            onChange={(event) => setSecret(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && secret) void submit({ secret });
            }}
          />
          <div className="ssh-secret-buttons">
            <Button className="ssh-secret-action" disabled={submitting} onClick={() => void submit({ cancelled: true })}>
              {t("Cancel", "取消")}
            </Button>
            <Button
              variant="primary"
              className="ssh-secret-action"
              disabled={submitting || !secret}
              onClick={() => void submit({ secret })}
            >
              {submitting ? t("Submitting", "正在提交") : t("Submit", "提交")}
            </Button>
          </div>
        </div>
      ) : null}
      {interactive && !isSecret ? (
        <div className="ssh-secret-buttons">
          <Button className="ssh-secret-action" disabled={submitting} onClick={() => void submit({ confirmed: false })}>
            {t("Decline", "拒绝")}
          </Button>
          <Button
            variant="primary"
            className="ssh-secret-action"
            disabled={submitting}
            onClick={() => void submit({ confirmed: true })}
          >
            {submitting ? t("Submitting", "正在提交") : t("Confirm", "确认")}
          </Button>
        </div>
      ) : null}
      {error ? <div className="ssh-secret-error">{error.message}</div> : null}
      {done ? <div className="ssh-secret-status">{t("Handled", "已处理")}</div> : null}
      {!done && !active ? (
        <div className="ssh-secret-status">{t("This request ended with the run.", "请求已随本轮运行结束。")}</div>
      ) : null}
    </div>
  );
}

/**
 * 返回不同征询类型的卡片标题。
 *
 * @param kind 征询类型
 * @param t 双语文本选择方法
 * @returns 卡片标题
 */
function titleOf(kind: SshSecretRequest["kind"], t: (en: string, zh: string) => string): string {
  switch (kind) {
    case "passphrase":
      return t("SSH key passphrase required", "需要私钥口令");
    case "password":
      return t("SSH password required", "需要登录密码");
    case "sudo_password":
      return t("sudo password required", "需要 sudo 密码");
    case "host_key":
      return t("Confirm host key", "确认主机指纹");
    case "danger_command":
      return t("Confirm dangerous command", "确认高危命令");
    default:
      return t("SSH input required", "需要 SSH 输入");
  }
}
