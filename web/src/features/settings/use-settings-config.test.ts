import { describe, expect, it } from "vitest";

// 轻量纯函数回归：非法 JSON 不应被当成可保存 payload 优先源
// resolveSavePayload 未导出，这里复刻保存优先级约定做文档化测试。

function resolveSavePayload(
  raw: string,
  draft: { ok: boolean } | null,
  rawParseError: string | null
): { ok: boolean } {
  if (!rawParseError) {
    try {
      const parsed = JSON.parse(raw) as { ok: boolean };
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) return parsed;
    } catch {
      // fall through
    }
  }
  if (draft) return draft;
  throw new Error("Configuration is not ready to save");
}

describe("settings save payload priority", () => {
  it("prefers valid raw over draft", () => {
    expect(resolveSavePayload('{"ok":true}', { ok: false }, null)).toEqual({ ok: true });
  });

  it("falls back to draft when raw is invalid", () => {
    expect(resolveSavePayload("{", { ok: true }, "Unexpected end")).toEqual({ ok: true });
  });
});
