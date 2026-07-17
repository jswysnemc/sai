import { type RefObject, useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";

export interface FollowOutputScrollState {
  following: boolean;
  showJump: boolean;
}

export interface ScrollMetrics {
  scrollTop: number;
  scrollHeight: number;
  clientHeight: number;
}

export interface OutputScrollTarget {
  scrollTop: number;
  scrollHeight: number;
}

const BOTTOM_THRESHOLD = 80;

/**
 * 判断滚动区域是否处于底部附近。
 *
 * @param metrics 当前滚动尺寸
 * @param threshold 距底容差
 * @returns 位于容差范围内时返回 true
 */
export function isNearOutputBottom(metrics: ScrollMetrics, threshold = BOTTOM_THRESHOLD): boolean {
  return metrics.scrollHeight - metrics.scrollTop - metrics.clientHeight < threshold;
}

/**
 * 将输出区域移动到当前内容底部。
 *
 * @param element 需要跟随最新内容的滚动区域
 * @returns 无返回值
 */
export function scrollOutputToBottom(element: OutputScrollTarget | null): void {
  if (element) element.scrollTop = element.scrollHeight;
}

/**
 * 根据滚动位置和用户操作意图计算自动跟随状态。
 *
 * @param current 当前跟随状态
 * @param metrics 当前滚动尺寸
 * @param userInitiated 本次滚动是否由用户主动触发
 * @returns 更新后的跟随状态
 */
export function resolveFollowOutputState(
  current: FollowOutputScrollState,
  metrics: ScrollMetrics,
  userInitiated: boolean
): FollowOutputScrollState {
  const atBottom = isNearOutputBottom(metrics);
  if (atBottom) return { following: true, showJump: false };
  if (userInitiated) return { following: false, showJump: true };
  return current.following ? current : { following: false, showJump: true };
}

/**
 * 管理流式输出的底部跟随行为，用户向上查看历史时立即暂停跟随。
 *
 * @param scrollContainerRef 消息滚动容器引用
 * @param contentSignal 思考、正文或工具输出更新信号
 * @param resetSignal 会话切换重置信号
 * @returns 回底按钮状态、跳转方法和暂停跟随方法
 */
export function useFollowOutputScroll(
  scrollContainerRef: RefObject<HTMLElement | null>,
  contentSignal: unknown,
  resetSignal: unknown
) {
  const stateRef = useRef<FollowOutputScrollState>({ following: true, showJump: false });
  const userIntentDeadlineRef = useRef(0);
  const [showJump, setShowJump] = useState(false);

  /** 同步内部状态和回底按钮。 */
  const commitState = useCallback((next: FollowOutputScrollState) => {
    stateRef.current = next;
    setShowJump(next.showJump);
  }, []);

  /** 在短时间窗口内标记滚动来自用户主动操作。 */
  const markUserIntent = useCallback(() => {
    userIntentDeadlineRef.current = performance.now() + 700;
  }, []);

  useEffect(() => {
    const element = scrollContainerRef.current;
    if (!element) return;
    const onScroll = () => {
      const userInitiated = performance.now() <= userIntentDeadlineRef.current;
      const next = resolveFollowOutputState(stateRef.current, element, userInitiated);
      commitState(next);
    };
    const onPointerDown = (event: PointerEvent) => {
      // 1. 仅滚动容器本身的指针操作可能是滚动条拖动，正文内点击不算抢占滚动
      if (event.target === element) markUserIntent();
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (["ArrowUp", "ArrowDown", "PageUp", "PageDown", "Home", "End", " "].includes(event.key)) markUserIntent();
    };
    element.addEventListener("wheel", markUserIntent, { passive: true });
    element.addEventListener("touchstart", markUserIntent, { passive: true });
    element.addEventListener("pointerdown", onPointerDown, { passive: true });
    element.addEventListener("keydown", onKeyDown);
    element.addEventListener("scroll", onScroll, { passive: true });
    return () => {
      element.removeEventListener("wheel", markUserIntent);
      element.removeEventListener("touchstart", markUserIntent);
      element.removeEventListener("pointerdown", onPointerDown);
      element.removeEventListener("keydown", onKeyDown);
      element.removeEventListener("scroll", onScroll);
    };
  }, [commitState, markUserIntent, scrollContainerRef]);

  useLayoutEffect(() => {
    const element = scrollContainerRef.current;
    if (stateRef.current.following) scrollOutputToBottom(element);
  }, [contentSignal, scrollContainerRef]);

  useLayoutEffect(() => {
    commitState({ following: true, showJump: false });
    scrollOutputToBottom(scrollContainerRef.current);
  }, [commitState, resetSignal, scrollContainerRef]);

  /** 平滑滚动到底部并恢复后续流式跟随。 */
  const jumpToBottom = useCallback(() => {
    const element = scrollContainerRef.current;
    if (!element) return;
    commitState({ following: true, showJump: false });
    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    element.scrollTo({ top: element.scrollHeight, behavior: reducedMotion ? "auto" : "smooth" });
  }, [commitState, scrollContainerRef]);

  /** 暂停自动跟随，供概览跳转等显式导航使用。 */
  const pauseFollowing = useCallback(() => {
    commitState({ following: false, showJump: true });
  }, [commitState]);

  return { showJump, jumpToBottom, pauseFollowing };
}
