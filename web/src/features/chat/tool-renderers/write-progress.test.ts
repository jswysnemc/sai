import { describe, expect, it } from "vitest";
import { streamedDiffCounts, streamedLineCount } from "./write-progress";

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

describe("streamedDiffCounts", () => {
  it("write_file content 按新增行累计", () => {
    expect(streamedDiffCounts('{"path":"a.rs","content":"l1\\nl2\\nl3')).toEqual({
      added: 3,
      removed: 0
    });
  });

  it("str_replace 分 old/new 计删与增", () => {
    expect(
      streamedDiffCounts('{"path":"a.rs","old_string":"a\\nb","new_string":"x\\ny\\nz"}')
    ).toEqual({ added: 3, removed: 2 });
    expect(streamedDiffCounts('{"path":"a.rs","old_string":"a\\nb')).toEqual({
      added: 0,
      removed: 2
    });
  });

  it("字段尚未出现时返回 null", () => {
    expect(streamedDiffCounts('{"path":"a.rs"')).toBeNull();
    expect(streamedDiffCounts("")).toBeNull();
  });
});
