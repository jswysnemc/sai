import { afterEach, describe, expect, it, vi } from "vitest";
import { readEditorWordWrap, writeEditorWordWrap } from "./editor-word-wrap";

const memory = new Map<string, string>();

describe("editor-word-wrap", () => {
  afterEach(() => {
    memory.clear();
    vi.unstubAllGlobals();
  });

  it("没有记录时使用调用方默认值", () => {
    vi.stubGlobal("localStorage", {
      getItem: (key: string) => memory.get(key) ?? null,
      setItem: (key: string, value: string) => { memory.set(key, value); }
    });
    expect(readEditorWordWrap(false)).toBe(false);
    expect(readEditorWordWrap(true)).toBe(true);
  });

  it("写入后代码和 Markdown 读到同一偏好", () => {
    vi.stubGlobal("localStorage", {
      getItem: (key: string) => memory.get(key) ?? null,
      setItem: (key: string, value: string) => { memory.set(key, value); }
    });
    writeEditorWordWrap(true);
    expect(readEditorWordWrap(false)).toBe(true);
    writeEditorWordWrap(false);
    expect(readEditorWordWrap(true)).toBe(false);
  });
});
