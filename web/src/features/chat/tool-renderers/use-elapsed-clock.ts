import { useEffect, useState } from "react";

/** 计时刷新间隔：耗时展示到 0.1 秒，间隔更短只会白白重渲染 */
const TICK_MS = 200;

/**
 * 为执行中的工具卡提供推进的时钟。
 *
 * 只在 active 为真时挂载定时器，工具一旦结束立即停表，
 * 因此一屏历史工具卡不会持有任何定时器。
 *
 * @param active 是否需要持续刷新
 * @returns 当前毫秒时间戳
 */
export function useElapsedClock(active: boolean): number {
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    if (!active) return;
    // 1. 挂载瞬间先对齐一次，避免沿用上一次停表时的旧值
    setNow(Date.now());
    const timer = window.setInterval(() => setNow(Date.now()), TICK_MS);
    return () => window.clearInterval(timer);
  }, [active]);

  return now;
}
