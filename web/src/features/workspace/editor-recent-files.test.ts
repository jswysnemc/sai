import { describe, expect, it } from "vitest";
import { readRecentFiles, rememberRecentFile } from "./editor-recent-files";

describe("editor recent files", () => {
  it("把最新打开的文件放在最前并去掉重复", () => {
    const store = new Map<string, string>();
    const storage = {
      getItem: (key: string) => store.get(key) ?? null,
      setItem: (key: string, value: string) => { store.set(key, value); }
    };
    Object.defineProperty(globalThis, "localStorage", { configurable: true, value: storage });
    rememberRecentFile("src/a.ts");
    rememberRecentFile("src/b.ts");
    rememberRecentFile("src/a.ts");
    expect(readRecentFiles()).toEqual(["src/a.ts", "src/b.ts"]);
  });
});
