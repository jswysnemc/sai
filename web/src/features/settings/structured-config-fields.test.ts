import { describe, expect, it } from "vitest";
import { mergeStructuredSecretArray } from "./structured-config-fields";

describe("structured config secret arrays", () => {
  it("保留隐藏值索引并替换可见条目", () => {
    const sentinel = "__SECRET__";

    expect(mergeStructuredSecretArray(
      ["$env:SEARCH_KEY", sentinel],
      ["$env:NEXT_SEARCH_KEY"],
      sentinel
    )).toEqual(["$env:NEXT_SEARCH_KEY", sentinel]);
  });

  it("删除隐藏值之前的可见条目时保留空槽", () => {
    const sentinel = "__SECRET__";

    expect(mergeStructuredSecretArray(
      ["$env:SEARCH_KEY", sentinel],
      [],
      sentinel
    )).toEqual(["", sentinel]);
  });
});
