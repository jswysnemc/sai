import { useCallback, useEffect, useState } from "react";

/** 复制成功提示的保留时长 */
const FEEDBACK_MS = 1_600;

/**
 * 提供带成功反馈的复制动作。
 *
 * 复制这类瞬时操作没有反馈就无法确认是否生效，
 * 因此把"已复制"状态与动作绑在一起，调用方只需据此切换图标。
 *
 * @returns copied 为是否处于已复制状态，copy 为执行复制的方法
 */
export function useCopyAction(): { copied: boolean; copy: (value: string) => void } {
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!copied) return;
    const timer = window.setTimeout(() => setCopied(false), FEEDBACK_MS);
    return () => window.clearTimeout(timer);
  }, [copied]);

  const copy = useCallback((value: string) => {
    if (!value) return;
    // 剪贴板不可用时静默失败：这是次级操作，不值得用报错打断阅读
    void navigator.clipboard.writeText(value).then(() => setCopied(true), () => undefined);
  }, []);

  return { copied, copy };
}
