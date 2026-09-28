import { describe, expect, it } from "vitest";
import { DEFAULT_TERMINAL_PREFERENCES, parseTerminalPreferences } from "./terminal-preferences";

describe("terminal display preferences", () => {
  it("recovers from malformed and unsupported local storage", () => {
    for (const raw of ["{", "null", "[]", '{"font":"toString","fontSizeRem":"14","scrollback":"forever"}']) {
      expect(parseTerminalPreferences(raw)).toEqual(DEFAULT_TERMINAL_PREFERENCES);
    }
  });
  it("limits memory use and keeps valid zero scrollback", () => {
    expect(parseTerminalPreferences('{"font":"system","fontSizeRem":8,"scrollback":1000000}')).toEqual({ font: "system", fontSizeRem: 1.5, scrollback: 50000 });
    expect(parseTerminalPreferences('{"font":"monospace","fontSizeRem":0.8125,"scrollback":0}')).toEqual({ font: "monospace", fontSizeRem: 0.8125, scrollback: 0 });
  });
});
