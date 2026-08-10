import { useEffect, useRef, useState } from "react";
import {
  enqueueFrame,
  framesToPlay,
  FRAME_HOLD_MS,
  type SummaryFrame
} from "./summary-frame-queue";
import { useReducedMotion } from "./use-reduced-motion";

type ToolSummaryTextProps = {
  /** 当前帧标识，变化时触发切换动画 */
  contentKey: string;
  /** 主文本 */
  primaryText: string;
  /** 副文本，等宽字体 */
  secondaryText?: string;
  /** 是否启用切换动画 */
  animate?: boolean;
};

/**
 * 渲染工具摘要文本，并在内容变化时按队列依次播出。
 *
 * 工具状态常在极短时间内连跳数次，直接替换文本会让用户只看到最后一帧。
 * 这里让每帧至少停留 800ms 依次播出；落后过多时跳到最新帧，
 * 保证显示状态不会明显滞后于真实状态。
 *
 * @param props 帧标识与主副文本
 * @returns 带切换动画的摘要文本
 */
export function ToolSummaryText({
  contentKey,
  primaryText,
  secondaryText = "",
  animate = false
}: ToolSummaryTextProps) {
  const reducedMotion = useReducedMotion();
  const enabled = animate && !reducedMotion;
  const [shown, setShown] = useState<SummaryFrame>({
    key: contentKey,
    primaryText,
    secondaryText
  });
  const queued = useRef<SummaryFrame[]>([]);
  const holdTimer = useRef<number | null>(null);
  const holdStartedAt = useRef(0);

  useEffect(() => {
    const next: SummaryFrame = { key: contentKey, primaryText, secondaryText };
    // 未启用动画时直接落地，不排队
    if (!enabled) {
      queued.current = [];
      setShown(next);
      return;
    }
    if (next.key === shown.key) return;
    // 上一帧仍在保留期内：入队等待，避免文本闪烁到无法阅读
    if (holdTimer.current !== null) {
      queued.current = enqueueFrame(queued.current, next);
      return;
    }
    setShown(next);
    holdStartedAt.current = Date.now();
    /** 保留期结束后播出下一帧。 */
    const advance = () => {
      holdTimer.current = null;
      const pending = framesToPlay(queued.current, Date.now() - holdStartedAt.current);
      const [head, ...rest] = pending;
      if (!head) {
        queued.current = [];
        return;
      }
      queued.current = rest;
      setShown(head);
      holdStartedAt.current = Date.now();
      holdTimer.current = window.setTimeout(advance, FRAME_HOLD_MS);
    };
    holdTimer.current = window.setTimeout(advance, FRAME_HOLD_MS);
  }, [contentKey, primaryText, secondaryText, enabled, shown.key]);

  useEffect(
    () => () => {
      if (holdTimer.current !== null) window.clearTimeout(holdTimer.current);
    },
    []
  );

  return (
    <span className="flex min-w-0 items-center gap-2">
      <span className="min-w-0 truncate">{shown.primaryText}</span>
      {shown.secondaryText ? (
        <span className="min-w-0 truncate font-mono text-ink-soft">{shown.secondaryText}</span>
      ) : null}
    </span>
  );
}
