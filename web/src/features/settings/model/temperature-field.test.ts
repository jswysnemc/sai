import { describe, expect, it } from "vitest";
import { formatTemperature, parseTemperature, roundTemperature } from "./temperature-field";

describe("formatTemperature", () => {
  it("renders a short decimal instead of the f32 artifact", () => {
    expect(formatTemperature(0.8999999761581421)).toBe("0.9");
    expect(formatTemperature(0.7)).toBe("0.7");
  });

  it("treats missing values as an empty field", () => {
    expect(formatTemperature(undefined)).toBe("");
    expect(formatTemperature(null)).toBe("");
  });
});

describe("parseTemperature", () => {
  it("treats blank input as unset", () => {
    expect(parseTemperature("")).toEqual({ ok: true, value: undefined });
    expect(parseTemperature("   ")).toEqual({ ok: true, value: undefined });
  });

  it("accepts in-range decimals", () => {
    expect(parseTemperature("0.9")).toEqual({ ok: true, value: 0.9 });
    expect(parseTemperature("0")).toEqual({ ok: true, value: 0 });
    expect(parseTemperature("2")).toEqual({ ok: true, value: 2 });
  });

  it("rejects out-of-range or invalid text", () => {
    expect(parseTemperature("2.1")).toEqual({ ok: false });
    expect(parseTemperature("-0.1")).toEqual({ ok: false });
    expect(parseTemperature("abc")).toEqual({ ok: false });
  });
});

describe("roundTemperature", () => {
  it("collapses binary float noise", () => {
    expect(roundTemperature(0.8999999761581421)).toBe(0.9);
  });
});
