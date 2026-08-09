import { describe, expect, it } from "vitest";
import { streamedLineCount } from "./write-progress";

describe("streamedLineCount", () => {
  it("统计 JSON 前缀里的转义换行数", () => {
    const preview = '{"path":"a.rs","content":"line1\\nline2\\nline3';
    expect(streamedLineCount(preview)).toBe(2);
  });

  it("空参数计为零行", () => {
    expect(streamedLineCount("")).toBe(0);
  });

  it("相邻转义换行不重复计数", () => {
    expect(streamedLineCount('"a\\n\\n\\nb"')).toBe(3);
  });

  it("不把真实换行误当作转义序列", () => {
    expect(streamedLineCount('{"patch":"@@\n+x"}')).toBe(0);
  });
});
