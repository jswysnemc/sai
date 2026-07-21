import { describe, expect, it } from "vitest";
import { currentComposerTip } from "./composer-tips";

describe("currentComposerTip", () => {
  it("returns non-empty localized tips that rotate by time", () => {
    const first = currentComposerTip("zh-CN", 0);
    const later = currentComposerTip("zh-CN", 8_000);
    expect(first.length).toBeGreaterThan(0);
    expect(later.length).toBeGreaterThan(0);
    // 不同时间槽应切换（列表长度 > 1）
    expect(first === later).toBe(false);
  });

  it("returns English tips for en locales", () => {
    const tip = currentComposerTip("en-US", 0);
    expect(tip).toMatch(/[A-Za-z]/);
  });

  it("keeps web tips free of TUI-only shortcuts", () => {
    const samples = Array.from({ length: 20 }, (_, i) => currentComposerTip("en-US", i * 8_000)).join("\n");
    expect(samples).not.toMatch(/\bTUI\b/);
    expect(samples).not.toMatch(/Prefix !/);
    expect(samples).not.toMatch(/Double Esc/);
    expect(samples).not.toMatch(/Ctrl\+O/);
    expect(samples).toMatch(/@|skill|lightbox|Settings|paperclip|model/i);
  });
});
