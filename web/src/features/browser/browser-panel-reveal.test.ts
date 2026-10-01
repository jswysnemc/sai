import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { WebEvent } from "../../api/contracts/sessions";
import { isBrowserToolName, resetBrowserPanelRevealForTests, revealBrowserPanelForEvent } from "./browser-panel-reveal";

/**
 * 构造工具开始事件。
 *
 * @param name 工具名
 * @param runId 运行标识
 * @param replayed 是否为历史补发
 * @returns 会话事件
 */
function toolStarted(name: string, runId = "run-1", replayed = false): WebEvent {
  return {
    replayed,
    sequence: 1,
    run_id: runId,
    workspace_id: "w",
    session_id: "s",
    timestamp: "",
    type: "tool.call.started",
    payload: { tool_id: "t", name, arguments: "{}" }
  };
}

describe("browser panel reveal", () => {
  const dispatched: string[] = [];
  let mobile = false;

  beforeEach(() => {
    resetBrowserPanelRevealForTests();
    dispatched.length = 0;
    mobile = false;
    vi.stubGlobal("window", {
      matchMedia: () => ({ matches: mobile }),
      dispatchEvent: (event: CustomEvent<{ tab: string }>) => {
        dispatched.push(event.detail.tab);
        return true;
      }
    });
    vi.stubGlobal("CustomEvent", class<T> {
      detail: T;
      constructor(_type: string, init: { detail: T }) {
        this.detail = init.detail;
      }
    });
  });

  afterEach(() => vi.unstubAllGlobals());

  it("每轮运行首次调用浏览器工具时只打开一次面板", () => {
    expect(revealBrowserPanelForEvent(toolStarted("browser_navigate"))).toBe(true);
    expect(revealBrowserPanelForEvent(toolStarted("browser_click"))).toBe(false);
    expect(revealBrowserPanelForEvent(toolStarted("browser_click", "run-2"))).toBe(true);
    expect(dispatched).toEqual(["browser", "browser"]);
  });

  it("忽略历史补发、非浏览器工具与移动端布局", () => {
    expect(revealBrowserPanelForEvent(toolStarted("browser_navigate", "run-1", true))).toBe(false);
    expect(revealBrowserPanelForEvent(toolStarted("web_search"))).toBe(false);
    mobile = true;
    expect(revealBrowserPanelForEvent(toolStarted("browser_navigate"))).toBe(false);
    expect(dispatched).toEqual([]);
    expect(isBrowserToolName("browser_snapshot")).toBe(true);
    expect(isBrowserToolName(42)).toBe(false);
  });
});
