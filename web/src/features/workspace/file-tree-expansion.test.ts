import { afterEach, describe, expect, it, vi } from "vitest";
import { parseExpandedDirectories, readExpandedDirectories, writeExpandedDirectories } from "./file-tree-expansion";

const memory = new Map<string, string>();

describe("file-tree-expansion", () => {
  afterEach(() => {
    memory.clear();
    vi.unstubAllGlobals();
  });

  it("损坏的记录按空集合处理", () => {
    expect(parseExpandedDirectories("nope", "workspace")).toEqual(new Set());
    expect(parseExpandedDirectories(JSON.stringify({ workspace: ["src", 1, ""] }), "workspace")).toEqual(new Set(["src"]));
    expect(parseExpandedDirectories(JSON.stringify({ other: ["src"] }), "workspace")).toEqual(new Set());
  });

  it("按工作区记住展开目录，互不影响", () => {
    vi.stubGlobal("localStorage", {
      getItem: (key: string) => memory.get(key) ?? null,
      setItem: (key: string, value: string) => { memory.set(key, value); }
    });
    writeExpandedDirectories("alpha", new Set(["src", "web/src"]));
    writeExpandedDirectories("beta", new Set(["docs"]));
    expect(readExpandedDirectories("alpha")).toEqual(new Set(["src", "web/src"]));
    expect(readExpandedDirectories("beta")).toEqual(new Set(["docs"]));
  });
});
