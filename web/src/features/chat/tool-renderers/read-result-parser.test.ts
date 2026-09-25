import { describe, expect, it } from "vitest";
import { parseImageReadNote, parseReadLines, parseReadTextPages } from "./read-result-parser";

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
    expect(pages[0].lineCount).toBe(0);
  });

  it("解析读取范围与截断信息", () => {
    const pages = parseReadTextPages(JSON.stringify({
      type: "text-page",
      path: "/tmp/big.rs",
      offset: 10,
      limit: 500,
      content: "10: a\n11: b",
      truncated: true,
      next: 12
    }));
    expect(pages[0].offset).toBe(10);
    expect(pages[0].limit).toBe(500);
    expect(pages[0].lineCount).toBe(2);
    expect(pages[0].truncated).toBe(true);
    expect(pages[0].next).toBe(12);
  });

  it("未截断时不产生续读起点", () => {
    const pages = parseReadTextPages(JSON.stringify({ type: "text-page", path: "/tmp/a.rs", offset: 1, content: "1: a" }));
    expect(pages[0].truncated).toBe(false);
    expect(pages[0].next).toBeNull();
  });

  it("解析制表符行号文本", () => {
    const pages = parseReadTextPages("7\tconst a = 1;\n8\tvalue: ok");
    expect(pages).toHaveLength(1);
    expect(pages[0].offset).toBe(7);
    expect(pages[0].lineCount).toBe(2);
    expect(pages[0].lines).toEqual([
      { number: 7, text: "const a = 1;" },
      { number: 8, text: "value: ok" }
    ]);
  });

  it("目录列表和图片说明不当成源码行", () => {
    expect(parseReadTextPages("Directory: /tmp\nEntries 1-1 of 1:\na.txt")).toEqual([]);
    expect(parseImageReadNote("[Image: source: /tmp/shot.png, image/png, 1KB, 8x4]")).toEqual({
      source: "/tmp/shot.png",
      summary: "source: /tmp/shot.png, image/png, 1KB, 8x4"
    });
  });
});
