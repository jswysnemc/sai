import { Check, Copy, GitBranch, Pencil, RotateCcw } from "lucide-react";
import type React from "react";
import { useEffect, useRef, useState } from "react";
import { useI18n } from "../../i18n/use-i18n";

type MessageActionsProps = {
  text: string;
  className?: string;
  timestamp?: string;
  onRetry?: () => void;
  /** 把对话切回本轮，之后发送的消息成为本轮的新分支 */
  onContinueFrom?: () => void;
  /** 改写本轮用户输入并作为新分支重新发送 */
  onEdit?: () => void;
  busy?: boolean;
  /** 附加操作，例如分支版本切换 */
  extra?: React.ReactNode;
};

/**
 * 消息操作行：时间、重试、编辑重发、从此处分支、复制原文。
 */
export function MessageActions({
  text,
  className,
  timestamp,
  onRetry,
  onContinueFrom,
  onEdit,
  busy,
  extra
}: MessageActionsProps) {
  const { locale, t } = useI18n();
  const [copied, setCopied] = useState(false);
  const timerRef = useRef<number | null>(null);

  useEffect(() => () => {
    if (timerRef.current !== null) window.clearTimeout(timerRef.current);
  }, []);

  const onCopy = async () => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      if (timerRef.current !== null) window.clearTimeout(timerRef.current);
      timerRef.current = window.setTimeout(() => setCopied(false), 1_600);
    } catch {
      setCopied(false);
    }
  };

  return (
    <div className={`message-actions${className ? ` ${className}` : ""}`}>
      {timestamp && <time className="message-timestamp">{formatTimestamp(timestamp, locale)}</time>}
      {extra}
      {onRetry && (
        <button type="button" className="message-copy" onClick={onRetry} aria-label={t("Retry turn", "重试本轮")} title={t("Retry turn", "重试本轮")} disabled={busy}>
          <RotateCcw size={13} />
        </button>
      )}
      {onEdit && (
        <button
          type="button"
          className="message-copy"
          onClick={onEdit}
          aria-label={t("Edit and resend", "编辑并重新发送")}
          title={t("Edit this message and resend it as a new branch", "编辑本条消息并作为新分支重新发送")}
          disabled={busy}
        >
          <Pencil size={13} />
        </button>
      )}
      {onContinueFrom && (
        <button
          type="button"
          className="message-copy"
          onClick={onContinueFrom}
          aria-label={t("Continue from this turn", "从这里继续")}
          title={t("Continue from this turn as a new branch", "从这里继续，形成新分支")}
          disabled={busy}
        >
          <GitBranch size={13} />
        </button>
      )}
      <button type="button" className="message-copy" onClick={onCopy} aria-label={t("Copy original message", "复制消息原文")} title={t("Copy original", "复制原文")}>
        {copied ? <Check size={13} /> : <Copy size={13} />}
      </button>
    </div>
  );
}

function formatTimestamp(value: string, locale: "en-US" | "zh-CN"): string {
  const parsed = Date.parse(value);
  if (!Number.isFinite(parsed)) return value;
  const date = new Date(parsed);
  const today = new Date();
  const sameDay = date.toDateString() === today.toDateString();
  const time = date.toLocaleTimeString(locale, { hour: "2-digit", minute: "2-digit" });
  return sameDay ? time : `${date.toLocaleDateString(locale)} ${time}`;
}
