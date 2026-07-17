import { describe, expect, it } from "vitest";
import { navigateInputHistory } from "./input-history";

describe("input history", () => {
  it("preserves and restores the current draft", () => {
    const first = navigateInputHistory(["one", "two"], { index: null, draft: "" }, "draft", "up")!;
    expect(first.value).toBe("two");
    const second = navigateInputHistory(["one", "two"], first.state, first.value, "down")!;
    expect(second).toEqual({ state: { index: null, draft: "" }, value: "draft" });
  });
});
