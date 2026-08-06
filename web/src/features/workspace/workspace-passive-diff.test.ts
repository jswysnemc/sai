import { afterEach, describe, expect, it, vi } from "vitest";
import { OPEN_WORKSPACE_DIFF_EVENT, openWorkspaceDiff } from "./workspace-passive-diff";

class TestCustomEvent<T> extends Event {
  readonly detail: T;

  constructor(type: string, init: { detail: T }) {
    super(type);
    this.detail = init.detail;
  }
}

afterEach(() => vi.unstubAllGlobals());

describe("openWorkspaceDiff", () => {
  it("只通过具体比较动作发送完整 Diff 载荷", () => {
    const dispatchEvent = vi.fn();
    vi.stubGlobal("CustomEvent", TestCustomEvent);
    vi.stubGlobal("window", { dispatchEvent });
    const detail = {
      path: "src/main.rs",
      source: "@@ -1 +1 @@\n-old\n+new",
      title: "main.rs"
    };

    openWorkspaceDiff(detail);

    const event = dispatchEvent.mock.calls[0]?.[0] as TestCustomEvent<typeof detail>;
    expect(event.type).toBe(OPEN_WORKSPACE_DIFF_EVENT);
    expect(event.detail).toEqual(detail);
  });
});
