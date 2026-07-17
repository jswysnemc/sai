import { describe, expect, it } from "vitest";
import { parseReadLines, parseReadTextPages } from "./read-result-parser";

describe("read result parser", () => {
  it("解析单文件文本分页", () => {
    const pages = parseReadTextPages(JSON.stringify({ type: "text-page", path: "/tmp/a.ts", offset: 7, limit: 2, content: "7: const a = 1;\n8: value: ok" }));
    expect(pages).toHaveLength(1);
    expect(pages[0].lines).toEqual([
      { number: 7, text: "const a = 1;" },
      { number: 8, text: "value: ok" }
    ]);
  });

  it("从批量结果中保留文本文件", () => {
    const pages = parseReadTextPages(JSON.stringify({
      type: "multi-text-page",
      results: [
        { type: "text-page", path: "/tmp/a.py", offset: 1, limit: 1, content: "1: print('a')" },
        { type: "directory-page", path: "/tmp", entries: ["a.py"] },
        { type: "error", path: "/tmp/missing", error: "missing" }
      ]
    }));
    expect(pages.map((page) => page.path)).toEqual(["/tmp/a.py"]);
  });

  it("保留没有行号前缀的内容", () => {
    expect(parseReadLines("plain\n2: source")).toEqual([
      { number: null, text: "plain" },
      { number: 2, text: "source" }
    ]);
  });

  it("保留空文本文件", () => {
    const pages = parseReadTextPages(JSON.stringify({ type: "text-page", path: "/tmp/empty.rs", content: "" }));
    expect(pages).toHaveLength(1);
    expect(pages[0].lines).toEqual([{ number: null, text: "" }]);
  });
});
