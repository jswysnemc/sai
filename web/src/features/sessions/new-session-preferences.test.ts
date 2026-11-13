import { afterEach, describe, expect, it, vi } from "vitest";
import type { AppConfig } from "../../api/contracts";
import {
  readStoredChatModelSelection,
  readStoredThinkingLevel,
  writeStoredChatModelSelection,
  writeStoredThinkingLevel
} from "../chat/session-preference-storage";
import {
  buildNewSessionModelChoices,
  buildNewSessionThinkingLevels,
  initializeNewSessionPreferences,
  renameNewSessionProviderReference,
  disabledNewSessionProviderName,
  resolveConfiguredNewSessionPreferences,
  resolveEffectiveNewSessionPreferences
} from "./new-session-preferences";

const nativeConfig = {
  active_provider: "provider-a",
  providers: [
    {
      id: "provider-a",
      display_name: "Provider A",
      base_url: "https://example.test/v1",
      models: ["model-a", "model-b"],
      default_model: "model-a"
    }
  ],
  session: {}
} as unknown as AppConfig;

afterEach(() => vi.unstubAllGlobals());

describe("new session preferences", () => {
  it("resolves an explicit model and normalizes an invalid thinking level", () => {
    const config = {
      ...nativeConfig,
      session: {
        new_session_provider_id: "provider-a",
        new_session_model: "model-b",
        new_session_thinking_level: "invalid"
      }
    } as unknown as AppConfig;

    expect(resolveConfiguredNewSessionPreferences(config)).toEqual({
      model: { providerId: "provider-a", model: "model-b" },
      thinkingLevel: "auto"
    });
    expect(disabledNewSessionProviderName(config)).toBeNull();
  });

  it("names the disabled provider that still owns the new-session model", () => {
    const config = {
      ...nativeConfig,
      providers: [{ ...nativeConfig.providers[0], enabled: false, display_name: "Provider A" }],
      session: {
        new_session_provider_id: "provider-a",
        new_session_model: "model-b"
      }
    } as unknown as AppConfig;
    expect(disabledNewSessionProviderName(config)).toBe("Provider A");
    expect(buildNewSessionModelChoices(config).map((choice) => choice.model)).toEqual([]);
  });

  it("writes only the new session keys and blocks an old global model fallback", () => {
    installLocalStorage();
    writeStoredChatModelSelection(undefined, { providerId: "provider-a", model: "global-model" });
    writeStoredThinkingLevel(undefined, "low");
    writeStoredChatModelSelection("old-session", { providerId: "provider-a", model: "old-model" });
    writeStoredThinkingLevel("old-session", "high");

    const initialized = initializeNewSessionPreferences("new-session", nativeConfig);

    expect(initialized).toEqual({ model: null, thinkingLevel: "auto" });
    expect(readStoredChatModelSelection("new-session")).toBeNull();
    expect(readStoredThinkingLevel("new-session")).toBe("auto");
    expect(readStoredChatModelSelection("old-session")).toEqual({
      providerId: "provider-a",
      model: "old-model"
    });
    expect(readStoredThinkingLevel("old-session")).toBe("high");
  });

  it("writes an explicitly configured model and thinking level", () => {
    installLocalStorage();
    const config = {
      ...nativeConfig,
      session: {
        new_session_provider_id: "provider-a",
        new_session_model: "model-b",
        new_session_thinking_level: "xhigh"
      }
    } as AppConfig;

    initializeNewSessionPreferences("fixed-session", config);

    expect(readStoredChatModelSelection("fixed-session")).toEqual({
      providerId: "provider-a",
      model: "model-b"
    });
    expect(readStoredThinkingLevel("fixed-session")).toBe("xhigh");
  });


});

/**
 * 【会话】【偏好测试】安装测试用内存 localStorage。
 *
 * @returns 底层键值映射
 */
function installLocalStorage(): Map<string, string> {
  const values = new Map<string, string>();
  vi.stubGlobal("window", {
    localStorage: {
      getItem: (key: string) => values.get(key) ?? null,
      setItem: (key: string, value: string) => values.set(key, value),
      removeItem: (key: string) => values.delete(key),
      clear: () => values.clear()
    }
  });
  return values;
}
