import { describe, expect, it } from "vitest";
import { composerTips, currentComposerTip } from "./composer-tips";

describe("currentComposerTip", () => {
  it("returns a stable tip for the same page load", () => {
    const first = currentComposerTip("zh-CN");
    const later = currentComposerTip("zh-CN");
    expect(first.length).toBeGreaterThan(0);
    expect(later).toBe(first);
  });

  it("returns English tips for en locales", () => {
    const tip = currentComposerTip("en-US");
    expect(tip).toMatch(/[A-Za-z]/);
  });

  it("keeps web tips free of TUI-only shortcuts", () => {
    const samples = composerTips("en-US").join("\n");
    expect(samples).not.toMatch(/\bTUI\b/);
    expect(samples).not.toMatch(/Prefix !/);
    expect(samples).not.toMatch(/Double Esc/);
    expect(samples).not.toMatch(/Ctrl\+O/);
    expect(samples).toMatch(/@|skill|lightbox|Settings|paperclip|model|rename/i);
  });
});
