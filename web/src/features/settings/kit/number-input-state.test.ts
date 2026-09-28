import { describe, expect, it } from "vitest";
import { parseNumberDraft, resolveNumberCommit } from "./number-input-state";

describe("settings number input commits", () => {
  it("keeps incomplete drafts out of the configuration", () => {
    for (const text of ["-", "1e", "NaN", "Infinity"]) expect(parseNumberDraft(text, {})).toEqual({ kind: "invalid" });
    expect(resolveNumberCommit("-", 90, { min: 50, max: 99 }, false)).toEqual({ value: undefined, display: "90" });
  });

  it("clamps out-of-range ratios only when committing", () => {
    const bounds = { min: 50, max: 99, integer: true };
    expect(parseNumberDraft("9", bounds)).toEqual({ kind: "value", value: 9, inRange: false });
    expect(resolveNumberCommit("9", 90, bounds, false)).toEqual({ value: 50, display: "50" });
    expect(resolveNumberCommit("100", 90, bounds, false)).toEqual({ value: 99, display: "99" });
  });

  it("distinguishes zero, required empty fields, and nullable fields", () => {
    expect(resolveNumberCommit("0", 200, { min: 0 }, false).value).toBe(0);
    expect(resolveNumberCommit("", 200, {}, false)).toEqual({ value: undefined, display: "200" });
    expect(resolveNumberCommit("", 200, {}, true)).toEqual({ value: null, display: "" });
    expect(resolveNumberCommit("1.5", 3, { integer: true }, false).value).toBeUndefined();
  });
});
