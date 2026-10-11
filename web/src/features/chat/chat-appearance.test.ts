import { beforeEach, describe, expect, it } from "vitest";
import {
  CHAT_APPEARANCE_STORAGE_KEY,
  DEFAULT_CHAT_APPEARANCE,
  getChatAppearanceStyle,
  normalizeChatAppearance,
  persistChatAppearance,
  readChatAppearance,
  type AppearanceStorage
} from "./chat-appearance";

function memoryStorage(initial: Record<string, string> = {}): AppearanceStorage & { data: Record<string, string> } {
  const data = { ...initial };
  return {
    data,
    getItem: (key) => data[key] ?? null,
    setItem: (key, value) => {
      data[key] = value;
    }
  };
}

describe("chat appearance preferences", () => {
  beforeEach(() => {
    // 每个用例使用独立存储，避免模块快照依赖浏览器
  });

  it.each([null, [], "serif", { font: "__proto__", width: "constructor", fontSize: Number.NaN }])(
    "rejects malformed preference values: %j",
    (value) => {
      expect(normalizeChatAppearance(value)).toEqual(DEFAULT_CHAT_APPEARANCE);
    }
  );

  it("bounds and rounds sizes and never interpolates arbitrary CSS", () => {
    expect(normalizeChatAppearance({ fontSize: 100 }).fontSize).toBe(24);
    expect(normalizeChatAppearance({ fontSize: 1 }).fontSize).toBe(12);
    expect(normalizeChatAppearance({ fontSize: 16.6 }).fontSize).toBe(17);
    expect(getChatAppearanceStyle({ font: "serif", fontSize: 20, width: "wide" })).toMatchObject({
      "--chat-font-size": "20px",
      "--chat-column-width": "75rem"
    });
    expect(getChatAppearanceStyle(DEFAULT_CHAT_APPEARANCE)).toMatchObject({
      "--chat-font-family": "var(--font-ui)",
      "--chat-font-size": "15px",
      "--chat-column-width": "50rem"
    });
    expect(getChatAppearanceStyle({ font: "mono", fontSize: 18, width: "full" })["--chat-column-width"]).toBe("100%");
  });

  it("round trips preferences while preserving unknown fields", () => {
    const storage = memoryStorage({
      [CHAT_APPEARANCE_STORAGE_KEY]: JSON.stringify({ version: 1, extra: "keep" })
    });
    persistChatAppearance({ font: "serif", fontSize: 18, width: "full" }, storage);
    expect(readChatAppearance(storage)).toEqual({ font: "serif", fontSize: 18, width: "full" });
    expect(JSON.parse(storage.data[CHAT_APPEARANCE_STORAGE_KEY])).toHaveProperty("extra", "keep");
  });

  it("preserves future schemas through reads and user changes", () => {
    const raw = JSON.stringify({ version: 2, font: "future-font", custom: [1, 2] });
    const storage = memoryStorage({ [CHAT_APPEARANCE_STORAGE_KEY]: raw });
    expect(readChatAppearance(storage)).toEqual(DEFAULT_CHAT_APPEARANCE);
    persistChatAppearance({ font: "mono", fontSize: 24, width: "full" }, storage);
    expect(storage.data[CHAT_APPEARANCE_STORAGE_KEY]).toBe(raw);
  });

  it("falls back safely when storage is corrupt or blocked", () => {
    const corrupt = memoryStorage({ [CHAT_APPEARANCE_STORAGE_KEY]: "{bad" });
    expect(readChatAppearance(corrupt)).toEqual(DEFAULT_CHAT_APPEARANCE);
    const blocked: AppearanceStorage = {
      getItem: () => {
        throw new Error("blocked");
      },
      setItem: () => {
        throw new Error("blocked");
      }
    };
    expect(readChatAppearance(blocked)).toEqual(DEFAULT_CHAT_APPEARANCE);
    expect(() => persistChatAppearance(DEFAULT_CHAT_APPEARANCE, blocked)).not.toThrow();
  });
});
