import { useEffect, useRef, useState } from "react";

/** 组停止增长多久后落到聚合标签 */
const SETTLE_DELAY_MS = 2_400;

type ToolGroupTickerProps = {
  /** 组内每项工具的单行摘要，按完成顺序排列 */
  items: string[];
  /** 静默后落定的聚合标签，如「探索了 4 个文件」 */
  aggregate: string;
  /** 是否处于实时轮次；历史回放与用户停止后直接展示聚合标签 */
  live: boolean;
};

/**
 * 渲染工具组折叠行的纵向单行轮播。
 *
 * 实时执行时每完成一项工具，它的摘要就把上一条向上顶走滚入原位；
 * 组停止增长一小段时间、轮次结束或被用户中断后，落定为聚合标签。
 * 历史加载的组不播动画——一屏旧记录同时开播只会让页面乱闪。
 *
 * @param props 工具摘要列表、聚合标签与实时标记
 * @returns 单行轮播
 */
export function ToolGroupTicker({ items, aggregate, live }: ToolGroupTickerProps) {
  const [settled, setSettled] = useState(!live);
  const lastCountRef = useRef(items.length);

  useEffect(() => {
    // 1. 轮次结束或被中断：立即落定，轮播随任务一起停
    if (!live) {
      setSettled(true);
      return;
    }
    // 2. 组每次增长都重新开始计时，静默一段时间后落到聚合标签
    if (items.length !== lastCountRef.current) {
      lastCountRef.current = items.length;
      setSettled(false);
    }
    const timer = window.setTimeout(() => setSettled(true), SETTLE_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [items.length, live]);

  const label = settled || items.length === 0 ? aggregate : items[items.length - 1];
  return <TickerLine label={label} />;
}

/**
 * 渲染带滚动过渡的单行文本。
 *
 * 文本变化时旧行向上滚出、新行自下滚入；两段都是一次性 CSS 动画，
 * 时序不依赖 JS 定时器。
 *
 * @param props label 为当前应展示的文本
 * @returns 滚动行
 */
function TickerLine({ label }: { label: string }) {
  const [current, setCurrent] = useState(label);
  const [leaving, setLeaving] = useState<string | null>(null);

  useEffect(() => {
    if (label === current) return;
    setLeaving(current);
    setCurrent(label);
  }, [label, current]);

  return (
    <span className="tool-group-ticker">
      {leaving !== null && (
        <span
          className="tool-group-ticker-line is-leaving"
          onAnimationEnd={() => setLeaving(null)}
          aria-hidden
        >
          {leaving}
        </span>
      )}
      <span key={current} className="tool-group-ticker-line is-entering">{current}</span>
    </span>
  );
}
