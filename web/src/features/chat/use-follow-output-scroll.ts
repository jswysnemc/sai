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

/** 回底动画时长，与距底远近无关，保证长短会话手感一致 */
const JUMP_DURATION_MS = 180;

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
 * 把滚动位置向下对齐到整行，避免限高容器贴底时顶边裁出半行字。
 *
 * @param scrollTop 目标滚动位置
 * @param lineHeight 行高（像素）
 * @param paddingTop 内容区上内边距
 * @param maxScroll 最大可滚动距离
 * @returns 对齐后的 scrollTop
 */
export function snapScrollTopToLine(
  scrollTop: number,
  lineHeight: number,
  paddingTop: number,
  maxScroll: number
): number {
  const bounded = Math.min(Math.max(0, maxScroll), Math.max(0, scrollTop));
  if (!Number.isFinite(lineHeight) || lineHeight <= 0) return bounded;
  const inset = Math.max(0, paddingTop);
  if (bounded <= inset) return 0;
  const snapped = Math.floor((bounded - inset) / lineHeight + 1e-6) * lineHeight + inset;
  return Math.min(Math.max(0, maxScroll), Math.max(0, snapped));
}

/**
 * 嵌套输出（长思考块）滚到最新内容，并按行高对齐顶边。
 *
 * @param element 限高滚动容器
 * @returns 无返回值
 */
export function scrollNestedOutputToBottom(element: HTMLElement | null): void {
  if (!element) return;
  const styles = getComputedStyle(element);
  const lineHeight = resolveLineHeightPx(styles);
  const paddingTop = Number.parseFloat(styles.paddingTop) || 0;
  const maxScroll = Math.max(0, element.scrollHeight - element.clientHeight);
  element.scrollTop = snapScrollTopToLine(maxScroll, lineHeight, paddingTop, maxScroll);
}

/**
 * 读取可用于行对齐的像素行高；`normal` 时按字号回退。
 *
 * @param styles 计算样式
 * @returns 行高像素，无法解析时返回 0
 */
function resolveLineHeightPx(styles: CSSStyleDeclaration): number {
  const parsed = Number.parseFloat(styles.lineHeight);
  if (Number.isFinite(parsed) && parsed > 0) return parsed;
  const fontSize = Number.parseFloat(styles.fontSize);
  return Number.isFinite(fontSize) && fontSize > 0 ? fontSize * 1.75 : 0;
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
 * 用户正在滚轮/触摸/按键时，禁止程序把视口拽回底部。
 *
 * 流式 token 的 layout 贴底若抢在 scroll 事件前执行，
 * 用户刚抬起的那一下会被立刻拉回去，跟随时窗形同虚设。
 *
 * @param following 当前是否跟随底部
 * @param intentDeadline 用户意图截止时间（performance.now 口径）
 * @param now 当前时间
 * @returns 此时是否仍允许程序贴底
 */
export function canProgrammaticFollow(
  following: boolean,
  intentDeadline: number,
  now: number
): boolean {
  return following && now > intentDeadline;
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
    if (!canProgrammaticFollow(stateRef.current.following, userIntentDeadlineRef.current, performance.now())) {
      return;
    }
    scrollOutputToBottom(element);
  }, [contentSignal, scrollContainerRef]);

  useLayoutEffect(() => {
    commitState({ following: true, showJump: false });
    scrollOutputToBottom(scrollContainerRef.current);
  }, [commitState, resetSignal, scrollContainerRef]);

  /**
   * 快速滚动到底部并恢复后续流式跟随。
   *
   * 浏览器原生 smooth 在长会话里会滚很久，这里改成固定时长的自定义缓动：
   * 无论距底多远都在同一段时间内到位，长短会话的手感一致。
   */
  const jumpToBottom = useCallback(() => {
    const element = scrollContainerRef.current;
    if (!element) return;
    commitState({ following: true, showJump: false });
    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const target = element.scrollHeight - element.clientHeight;
    if (reducedMotion) {
      element.scrollTop = target;
      return;
    }
    const start = element.scrollTop;
    const distance = target - start;
    if (Math.abs(distance) < 1) return;
    const startedAt = performance.now();
    const step = (now: number) => {
      const progress = Math.min(1, (now - startedAt) / JUMP_DURATION_MS);
      // easeOutCubic：起步快、收尾稳，不会有 smooth 那种长尾
      const eased = 1 - (1 - progress) ** 3;
      element.scrollTop = start + distance * eased;
      // 内容仍在流式增长时跟住新的底部
      if (progress < 1) requestAnimationFrame(step);
      else element.scrollTop = element.scrollHeight - element.clientHeight;
    };
    requestAnimationFrame(step);
  }, [commitState, scrollContainerRef]);

  /** 暂停自动跟随，供概览跳转等显式导航使用。 */
  const pauseFollowing = useCallback(() => {
    commitState({ following: false, showJump: true });
  }, [commitState]);

  return { showJump, jumpToBottom, pauseFollowing };
}

/**
 * 管理嵌套输出区域（如长思考块）的底部跟随。
 * 用户主动上滚查看历史时暂停；滚回底部后恢复自动跟随。
 *
 * @param scrollContainerRef 嵌套滚动容器引用
 * @param contentSignal 内容增长信号
 * @param enabled 是否启用跟随（通常为 live 且展开）
 * @returns 无返回值
 */
export function useNestedFollowOutputScroll(
  scrollContainerRef: RefObject<HTMLElement | null>,
  contentSignal: unknown,
  enabled: boolean
): void {
  const followingRef = useRef(true);
  const userIntentDeadlineRef = useRef(0);

  // 1. 重新启用时恢复跟随，避免上次上滚状态残留
  useEffect(() => {
    if (enabled) followingRef.current = true;
  }, [enabled]);

  useEffect(() => {
    const element = scrollContainerRef.current;
    if (!element || !enabled) return;

    /**
     * 在短时间窗口内标记滚动来自用户主动操作。
     *
     * @returns 无返回值
     */
    const markUserIntent = () => {
      userIntentDeadlineRef.current = performance.now() + 700;
    };

    /**
     * 根据当前位置更新是否继续跟随底部。
     *
     * @returns 无返回值
     */
    const onScroll = () => {
      const userInitiated = performance.now() <= userIntentDeadlineRef.current;
      const next = resolveFollowOutputState(
        { following: followingRef.current, showJump: !followingRef.current },
        {
          scrollTop: element.scrollTop,
          scrollHeight: element.scrollHeight,
          clientHeight: element.clientHeight
        },
        userInitiated
      );
      followingRef.current = next.following;
    };

    /**
     * 仅容器本身的指针操作可能是滚动条拖动。
     *
     * @param event 指针事件
     * @returns 无返回值
     */
    const onPointerDown = (event: PointerEvent) => {
      if (event.target === element) markUserIntent();
    };

    /**
     * 键盘滚动视为用户意图。
     *
     * @param event 键盘事件
     * @returns 无返回值
     */
    const onKeyDown = (event: KeyboardEvent) => {
      if (["ArrowUp", "ArrowDown", "PageUp", "PageDown", "Home", "End", " "].includes(event.key)) {
        markUserIntent();
      }
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
  }, [enabled, scrollContainerRef]);

  // 2. 仅在跟随开启且用户没有正在滚动时把视口钉在最新内容
  useLayoutEffect(() => {
    if (!enabled) return;
    if (!canProgrammaticFollow(followingRef.current, userIntentDeadlineRef.current, performance.now())) {
      return;
    }
    scrollNestedOutputToBottom(scrollContainerRef.current);
  }, [contentSignal, enabled, scrollContainerRef]);
}
