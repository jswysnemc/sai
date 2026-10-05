import { describe, expect, it } from "vitest";
import { browserViewportSize } from "./browser-viewport-size";

describe("browserViewportSize", () => {
  it("follows a small panel instead of clamping to 320x240", () => {
    expect(browserViewportSize({ width: 180, height: 120 })).toEqual({ width: 180, height: 120 });
  });

  it("ignores hidden panels", () => {
    expect(browserViewportSize({ width: 20, height: 400 })).toBeNull();
  });
});
