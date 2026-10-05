import { describe, expect, it } from "vitest";
import { lineNumberDigitsStyle } from "./line-number-gutter";

describe("lineNumberDigitsStyle", () => {
  it("按最大行号位数定宽，至少两位", () => {
    expect(lineNumberDigitsStyle(3)).toEqual({ "--line-number-digits": 2 });
    expect(lineNumberDigitsStyle(11)).toEqual({ "--line-number-digits": 2 });
    expect(lineNumberDigitsStyle(120)).toEqual({ "--line-number-digits": 3 });
    expect(lineNumberDigitsStyle(0)).toEqual({ "--line-number-digits": 2 });
  });
});
