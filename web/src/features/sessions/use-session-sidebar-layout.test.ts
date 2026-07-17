import { describe, expect, it } from "vitest";
import {
  SESSION_SIDEBAR_DEFAULT_WIDTH,
  clampSessionSidebarWidth,
  parseSessionSidebarWidth
} from "./use-session-sidebar-layout";

describe("session sidebar layout", () => {
  it("限制侧栏宽度范围", () => {
    expect(clampSessionSidebarWidth(120)).toBe(190);
    expect(clampSessionSidebarWidth(300)).toBe(300);
    expect(clampSessionSidebarWidth(520)).toBe(420);
  });

  it("解析持久化宽度并处理无效值", () => {
    expect(parseSessionSidebarWidth("318")).toBe(318);
    expect(parseSessionSidebarWidth("900")).toBe(420);
    expect(parseSessionSidebarWidth("invalid")).toBe(SESSION_SIDEBAR_DEFAULT_WIDTH);
    expect(parseSessionSidebarWidth(null)).toBe(SESSION_SIDEBAR_DEFAULT_WIDTH);
  });
});
