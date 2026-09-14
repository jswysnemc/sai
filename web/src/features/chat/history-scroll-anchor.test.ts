import { describe, expect, it } from "vitest";
import { captureHistoryScroll, restoreHistoryScroll } from "./history-scroll-anchor";

/**
 * 【会话载入】【位置样本】模拟历史区块展开前后的滚动几何。
 * @param atBottom 是否从底部开始
 * @returns 可变高度与位置样本
 */
function geometry(atBottom = false) {
  const initialTop = atBottom ? 3400 : 1000;
  let shift = 0;
  const container = {
    scrollTop: initialTop, scrollHeight: 4000, clientHeight: 600,
    contains: () => true,
    getBoundingClientRect: () => ({ top: 100 }),
  };
  const anchor = {
    isConnected: true,
    nextElementSibling: null,
    getBoundingClientRect: () => ({ top: 150 + shift - (container.scrollTop - initialTop) }),
  };
  const section = {
    closest: () => container,
    nextElementSibling: anchor,
    getBoundingClientRect: () => ({ top: 50 }),
  } as unknown as HTMLElement;
  return { container, section, grow: (height: number) => { shift += height; container.scrollHeight += height; } };
}

describe("history scroll anchors", () => {
  it("preserves the visible position when older content grows", () => {
    const fixture = geometry();
    const snapshot = captureHistoryScroll(fixture.section);
    fixture.grow(592);
    restoreHistoryScroll(snapshot);
    expect(fixture.container.scrollTop).toBe(1592);
  });

  it("does not compensate twice when multiple history blocks mount together", () => {
    const fixture = geometry();
    const first = captureHistoryScroll(fixture.section);
    const second = captureHistoryScroll(fixture.section);
    fixture.grow(592);
    restoreHistoryScroll(second);
    restoreHistoryScroll(first);
    expect(fixture.container.scrollTop).toBe(1592);
  });

  it("keeps user scrolling that happened while content was rendering", () => {
    const fixture = geometry();
    const snapshot = captureHistoryScroll(fixture.section);
    fixture.container.scrollTop = 900;
    fixture.grow(592);
    restoreHistoryScroll(snapshot);
    expect(fixture.container.scrollTop).toBe(900);
  });

  it("keeps the latest content at the bottom", () => {
    const fixture = geometry(true);
    const snapshot = captureHistoryScroll(fixture.section);
    fixture.grow(592);
    restoreHistoryScroll(snapshot);
    expect(fixture.container.scrollTop).toBe(fixture.container.scrollHeight);
  });
});
