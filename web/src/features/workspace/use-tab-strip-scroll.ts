import { useEffect, type RefObject } from "react";

/**
 * 管理工作区标签条的横向滚动行为。
 *
 * 溢出标记驱动两端渐隐遮罩；当前标签只在切换或增删标签时滚入视野，
 * 用户手动滚动期间不会被拉回；鼠标纵向滚轮转为横向滚动，
 * 否则隐藏滚动条后只有触控板能看到被挤出的标签。
 *
 * @param scrollerRef 标签滚动容器
 * @param activeTabId 当前标签标识，变化时把该标签滚入视野
 * @param tabs 标签列表，变化时重新计算溢出与可见位置
 * @returns 无返回值
 */
export function useTabStripScroll(scrollerRef: RefObject<HTMLElement | null>, activeTabId: string | null, tabs: readonly unknown[]) {
  useEffect(() => {
    const scroller = scrollerRef.current;
    if (!scroller) return;
    /** 【Web 工作台】【面板标签】按滚动位置标记左右是否还有被遮住的标签。 */
    const updateOverflow = () => {
      const overflow = scroller.scrollWidth - scroller.clientWidth > 1;
      scroller.dataset.overflowLeft = overflow && scroller.scrollLeft > 1 ? "true" : "false";
      scroller.dataset.overflowRight = overflow && scroller.scrollLeft + scroller.clientWidth < scroller.scrollWidth - 1 ? "true" : "false";
    };
    /**
     * 把纵向滚轮换算为横向滚动。
     *
     * @param event 滚轮事件
     */
    const handleWheel = (event: WheelEvent) => {
      // 1. 触控板横向手势、按住 Shift 的滚动交给浏览器原生处理
      if (event.shiftKey || Math.abs(event.deltaX) >= Math.abs(event.deltaY)) return;
      // 2. 标签未溢出时不拦截，页面其余滚动不受影响
      if (scroller.scrollWidth - scroller.clientWidth <= 1) return;
      event.preventDefault();
      scroller.scrollLeft += event.deltaY;
    };
    updateOverflow();
    scroller.addEventListener("scroll", updateOverflow, { passive: true });
    scroller.addEventListener("wheel", handleWheel, { passive: false });
    const observer = new ResizeObserver(updateOverflow);
    observer.observe(scroller);
    return () => {
      scroller.removeEventListener("scroll", updateOverflow);
      scroller.removeEventListener("wheel", handleWheel);
      observer.disconnect();
    };
  }, [scrollerRef, tabs]);

  useEffect(() => {
    // 只在切换或增删标签时对齐当前标签，避免与用户手动滚动相互拉扯
    revealTab(scrollerRef.current?.querySelector<HTMLElement>('[aria-selected="true"]'));
  }, [scrollerRef, activeTabId, tabs]);
}

/**
 * 把页签整体滚入视野。
 *
 * 选中态标在页签主按钮上，关闭按钮是它的兄弟节点；
 * 只滚动主按钮时末尾页签的关闭按钮会留在遮罩外，因此取外层页签容器。
 *
 * @param tabButton 页签主按钮
 * @returns 无返回值
 */
export function revealTab(tabButton: HTMLElement | null | undefined) {
  (tabButton?.closest<HTMLElement>(".workspace-tab") ?? tabButton)
    ?.scrollIntoView({ block: "nearest", inline: "nearest" });
}
