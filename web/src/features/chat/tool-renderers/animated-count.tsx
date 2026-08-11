import { useEffect, useRef, useState } from "react";

/**
 * 渲染朝目标值滚动逼近的数字。
 *
 * 流式写入时行数不断增长，数字直接跳变没有"正在进行"的感觉；
 * 这里在每次目标变化时用 rAF 分若干帧追上去。active 关闭（工具结束
 * 或被用户中断）时立即钉在最终值并取消未完成的帧，动效随任务一起停。
 *
 * 进行中挂载时从 0 起跳，避免首次渲染直接钉在目标值、看不见跳动。
 *
 * @param props value 为目标数值，active 表示是否仍在进行
 * @returns 当前展示的数字
 */
export function AnimatedCount({ value, active }: { value: number; active: boolean }) {
  const [shown, setShown] = useState(() => (active ? 0 : value));
  const shownRef = useRef(active ? 0 : value);
  const frameRef = useRef(0);

  useEffect(() => {
    const reduced = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
    // 1. 结束、被中断或用户要求减少动效时直接钉住目标值
    if (!active || reduced) {
      window.cancelAnimationFrame(frameRef.current);
      shownRef.current = value;
      setShown(value);
      return;
    }
    // 2. 每帧走完剩余距离的一部分，接近目标时收敛为逐一递增
    const step = () => {
      const current = shownRef.current;
      if (current === value) return;
      const delta = Math.max(1, Math.ceil(Math.abs(value - current) / 6));
      const next = current < value ? Math.min(value, current + delta) : Math.max(value, current - delta);
      shownRef.current = next;
      setShown(next);
      if (next !== value) frameRef.current = window.requestAnimationFrame(step);
    };
    window.cancelAnimationFrame(frameRef.current);
    frameRef.current = window.requestAnimationFrame(step);
    return () => window.cancelAnimationFrame(frameRef.current);
  }, [value, active]);

  return <>{shown}</>;
}
