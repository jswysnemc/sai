import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { captureHistoryScroll, restoreHistoryScroll, type HistoryScrollAnchor } from "./history-scroll-anchor";

type DeferredHistoryTurnProps = {
  turnId: string;
  eager: boolean;
  estimatedHeightRem: number;
  children: ReactNode;
};

/**
 * 【会话载入】【可见历史】先渲染近期轮次，旧轮次进入视口附近后再挂载正文。
 * @param props 轮次标识、首屏标记、占位高度和完整内容
 * @returns 保留导航锚点的轮次；已经挂载的内容保持挂载，保留工具展开状态
 */
export function DeferredHistoryTurn({ turnId, eager, estimatedHeightRem, children }: DeferredHistoryTurnProps) {
  const sectionRef = useRef<HTMLElement>(null);
  const anchorRef = useRef<HistoryScrollAnchor | null>(null);
  const [loaded, setLoaded] = useState(eager);
  const visible = eager || loaded;

  useEffect(() => {
    if (loaded) return;
    const section = sectionRef.current;
    if (!section) return;
    /** 【会话载入】【挂载准备】捕获当前阅读位置后启用正文；无参数，无返回值。 */
    const activate = () => {
      anchorRef.current = captureHistoryScroll(section);
      setLoaded(true);
    };
    if (eager || typeof IntersectionObserver === "undefined") {
      activate();
      return;
    }
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting)) {
        activate();
        observer.disconnect();
      }
    }, { root: section.closest(".message-scroll"), rootMargin: "480px 0px" });
    observer.observe(section);
    return () => observer.disconnect();
  }, [eager, loaded]);

  useLayoutEffect(() => {
    restoreHistoryScroll(anchorRef.current);
    anchorRef.current = null;
  }, [loaded]);

  return (
    <section
      ref={sectionRef}
      className="conversation-turn"
      data-overview-id={`turn-${turnId}`}
      data-history-loaded={visible}
    >
      {visible ? children : <div aria-hidden style={{ minBlockSize: `${estimatedHeightRem}rem` }} />}
    </section>
  );
}
