import { describe, expect, it } from "vitest";
import { dropDirectory, isNestedDrop } from "./file-tree-drag";

describe("file tree drag", () => {
  it("rejects dropping a folder onto itself or a child", () => {
    expect(isNestedDrop("src", "src")).toBe(true);
    expect(isNestedDrop("src", "src/lib")).toBe(true);
    expect(isNestedDrop("src/lib", "src")).toBe(false);
    expect(isNestedDrop("src", "")).toBe(false);
  });

  it("uses the file parent as the drop directory", () => {
    expect(dropDirectory("src/main.rs", false)).toBe("src");
    expect(dropDirectory("src", true)).toBe("src");
    expect(dropDirectory("main.rs", false)).toBe("");
  });
});
