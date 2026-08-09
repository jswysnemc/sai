import { describe, expect, it } from "vitest";
import { mergeSecretText, mergeSecretValues } from "./merge-secret-values";

describe("merge secret values", () => {
  const sentinel = "__SECRET__";

  it("保留隐藏值索引并替换可见条目", () => {
    expect(mergeSecretValues(
      ["$env:SEARCH_KEY", sentinel],
      ["$env:NEXT_SEARCH_KEY"],
      sentinel
    )).toEqual(["$env:NEXT_SEARCH_KEY", sentinel]);
  });

  it("删除隐藏值之前的可见条目时保留空槽", () => {
    expect(mergeSecretValues(["$env:SEARCH_KEY", sentinel], [], sentinel)).toEqual(["", sentinel]);
  });

  it("多出的可见条目追加到末尾", () => {
    expect(mergeSecretValues([sentinel], ["extra"], sentinel)).toEqual([sentinel, "extra"]);
  });

  it("无占位符时直接采用可见条目", () => {
    expect(mergeSecretValues(["a", "b"], ["c"], "")).toEqual(["c"]);
  });

  it("按换行与逗号解析文本后合并", () => {
    expect(mergeSecretText([sentinel, "old"], "new-1\nnew-2, new-3", sentinel)).toEqual([
      sentinel,
      "new-1",
      "new-2",
      "new-3"
    ]);
  });
});
