import { Check, Copy, GitBranch, RotateCcw } from "lucide-react";
import { useEffect, useRef, useState } from "react";

type MessageActionsProps = {
  text: string;
  timestamp?: string;
  onRetry?: () => void;
  onFork?: () => void;
  busy?: boolean;
};

/**
 * 消息操作行：时间、重试、分支、复制。
 */
export function MessageActions({ text, timestamp, onRetry, onFork, busy }: MessageActionsProps) {
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
    <div className="message-actions">
      {timestamp && <time className="message-timestamp">{formatTimestamp(timestamp)}</time>}
      {onRetry && (
        <button type="button" className="message-copy" onClick={onRetry} aria-label="重试本轮" title="重试本轮" disabled={busy}>
          <RotateCcw size={13} />
        </button>
      )}
      {onFork && (
        <button type="button" className="message-copy" onClick={onFork} aria-label="分支对话" title="分支对话" disabled={busy}>
          <GitBranch size={13} />
        </button>
      )}
      <button type="button" className="message-copy" onClick={onCopy} aria-label="复制消息原文" title="复制原文">
        {copied ? <Check size={13} /> : <Copy size={13} />}
      </button>
    </div>
  );
}

function formatTimestamp(value: string): string {
  const parsed = Date.parse(value);
  if (!Number.isFinite(parsed)) return value;
  const date = new Date(parsed);
  const today = new Date();
  const sameDay = date.toDateString() === today.toDateString();
  const time = date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  return sameDay ? time : `${date.toLocaleDateString()} ${time}`;
}
