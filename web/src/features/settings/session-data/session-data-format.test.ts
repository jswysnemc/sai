import { describe, expect, it } from "vitest";
import { formatSessionBytes, formatSessionDate } from "./session-data-format";

describe("formatSessionBytes", () => {
  it("按容量选择紧凑单位", () => {
    expect(formatSessionBytes(0)).toBe("0 B");
    expect(formatSessionBytes(512)).toBe("512 B");
    expect(formatSessionBytes(1536)).toBe("1.5 KiB");
    expect(formatSessionBytes(12 * 1024)).toBe("12 KiB");
  });
});

describe("formatSessionDate", () => {
  it("无效时间保持原文", () => {
    expect(formatSessionDate("unknown", "zh-CN")).toBe("unknown");
  });
});
