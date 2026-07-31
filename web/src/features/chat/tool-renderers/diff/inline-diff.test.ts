import { describe, expect, it } from "vitest";
import { annotateInlineDiff, segmentLinePair } from "./inline-diff";
import type { DiffLine } from "./diff-model";

/**
 * 提取被标记为改动的文本。
 *
 * @param segments 分段结果
 * @returns 改动片段拼接后的文本
 */
function changedText(segments: { text: string; changed: boolean }[] | undefined): string {
  return (segments ?? []).filter((item) => item.changed).map((item) => item.text).join("");
}

describe("inline diff", () => {
  it("highlights only the characters that differ", () => {
    const pair = segmentLinePair("const a = 1;", "const a = 2;");

    expect(changedText(pair.before)).toBe("1");
    expect(changedText(pair.after)).toBe("2");
  });

  it("keeps common prefix and suffix untouched", () => {
    const pair = segmentLinePair("value = compute(x)", "value = compute(y)");

    expect(pair.before.map((item) => item.text).join("")).toBe("value = compute(x)");
    expect(changedText(pair.before)).toBe("x");
  });

  it("does not split emoji or combining sequences", () => {
    const pair = segmentLinePair("status 🚀 ok", "status 🎯 ok");

    // 代理对被从中间切断会渲染成乱码
    expect(changedText(pair.before)).toBe("🚀");
    expect(changedText(pair.after)).toBe("🎯");
  });

  it("treats a full rewrite as one changed span", () => {
    const pair = segmentLinePair("alpha", "gamma");

    expect(changedText(pair.before)).toBe("alph");
    expect(changedText(pair.after)).toBe("gamm");
  });

  it("still strips a shared suffix in an otherwise full rewrite", () => {
    // alpha 与 beta 共享结尾的 a，最小差异应当保留它
    const pair = segmentLinePair("alpha", "beta");

    expect(changedText(pair.before)).toBe("alph");
    expect(changedText(pair.after)).toBe("bet");
  });

  it("handles insertion into an empty line", () => {
    const pair = segmentLinePair("", "added");

    expect(changedText(pair.before)).toBe("");
    expect(changedText(pair.after)).toBe("added");
  });

  it("pairs runs positionally without a similarity gate", () => {
    const lines: DiffLine[] = [
      { kind: "removed", text: "one" },
      { kind: "removed", text: "two" },
      { kind: "added", text: "1" },
      { kind: "added", text: "2" }
    ];

    const annotated = annotateInlineDiff(lines);

    // 第 i 个删除与第 i 个新增配对，与内容相似度无关
    expect(changedText(annotated[0].segments)).toBe("one");
    expect(changedText(annotated[2].segments)).toBe("1");
    expect(changedText(annotated[1].segments)).toBe("two");
    expect(changedText(annotated[3].segments)).toBe("2");
  });

  it("leaves unpaired lines without segments", () => {
    const lines: DiffLine[] = [
      { kind: "removed", text: "gone" },
      { kind: "removed", text: "also gone" },
      { kind: "added", text: "kept" }
    ];

    const annotated = annotateInlineDiff(lines);

    expect(annotated[0].segments).toBeDefined();
    expect(annotated[1].segments).toBeUndefined();
  });

  it("does not pair across context lines", () => {
    const lines: DiffLine[] = [
      { kind: "removed", text: "a" },
      { kind: "context", text: "middle" },
      { kind: "added", text: "b" }
    ];

    const annotated = annotateInlineDiff(lines);

    expect(annotated[0].segments).toBeUndefined();
    expect(annotated[2].segments).toBeUndefined();
  });
});
