import { useEffect, useState, type RefObject } from "react";

/** 收起按钮在视口中的悬浮位置。 */
export type CollapseAnchor = {
  /** 距视口顶部的距离 */
  top: number;
  /** 距视口右缘的距离，按钮据此右对齐并向左展开 */
  right: number;
};

/** 正文顶部滚出视口多远后才显示收起按钮 */
const REVEAL_THRESHOLD = 160;

/**
 * 计算展开后的系统提示词收起按钮位置，并决定何时显露。
 *
 * 提示词正文很长，滚到中段时标题栏上的折叠入口已经离开视野，因此在视口里
 * 悬浮一个收起按钮。刚展开时正文顶部就在眼前，此时按钮属于冗余，
 * 只有向下滚过一段距离后才出现。
 *
 * @param bodyRef 提示词正文容器引用，用于对齐右缘并测量滚动距离
 * @param open 提示词是否处于展开态
 * @returns 悬浮位置；不该显示时为 null
 */
export function useCollapseAnchor(
  bodyRef: RefObject<HTMLElement | null>,
  open: boolean
): CollapseAnchor | null {
  const [anchor, setAnchor] = useState<CollapseAnchor | null>(null);

  useEffect(() => {
    if (!open) {
      setAnchor(null);
      return;
    }

    /**
     * 同步按钮位置与显隐。
     *
     * 1. 正文顶部尚未滚过阈值时不显示，避免与标题栏的折叠入口重复
     * 2. 纵坐标固定在标题栏下方，横坐标贴齐正文右缘内侧
     *
     * @returns 无返回值
     */
    const sync = () => {
      const body = bodyRef.current;
      if (!body) return;
      const rect = body.getBoundingClientRect();
      const header = document.querySelector(".chat-header") as HTMLElement | null;
      const headerBottom = header?.getBoundingClientRect().bottom ?? 44;
      const scrolledPast = headerBottom - rect.top;
      // 正文已经整体滚出视口时同样收起按钮，避免悬停在无关内容之上
      if (scrolledPast < REVEAL_THRESHOLD || rect.bottom < headerBottom) {
        setAnchor(null);
        return;
      }
      const inset = 8;
      setAnchor({
        top: Math.max(headerBottom + 8, 12),
        right: Math.max(inset, window.innerWidth - rect.right + inset)
      });
    };

    sync();
    window.addEventListener("resize", sync);
    // 捕获阶段监听，消息区自身滚动也能触发
    window.addEventListener("scroll", sync, true);
    const observer = typeof ResizeObserver !== "undefined"
      ? new ResizeObserver(() => sync())
      : null;
    if (observer && bodyRef.current) observer.observe(bodyRef.current);
    return () => {
      window.removeEventListener("resize", sync);
      window.removeEventListener("scroll", sync, true);
      observer?.disconnect();
    };
  }, [bodyRef, open]);

  return anchor;
}
