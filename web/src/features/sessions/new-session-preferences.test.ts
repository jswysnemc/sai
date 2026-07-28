import { afterEach, describe, expect, it, vi } from "vitest";
import type { AppConfig, EngineStatusResponse } from "../../api/contracts";
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
  resetNewSessionEnginePreferences,
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
  agent: { engine: "native" },
  session: {}
} as unknown as AppConfig;

const connectedCodexStatus = {
  engine: "codex",
  label: "Codex",
  external: true,
  unavailable_features: [],
  acp_runtime: {
    connected: true,
    agent_name: "Codex",
    agent_version: "1.0.0",
    capabilities: null,
    auth_methods: [],
    modes: null,
    available_commands: [],
    native_equivalents: {},
    config_options: [
      {
        id: "model",
        category: "model",
        type: "select",
        currentValue: "runtime-model",
        options: [{ value: "runtime-model", name: "Runtime model" }]
      },
      {
        id: "thought",
        category: "thought_level",
        type: "select",
        currentValue: "high",
        options: [{ value: "high", name: "High" }]
      }
    ]
  }
} as EngineStatusResponse;

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
  });

  it("uses only models currently advertised by a connected ACP runtime", () => {
    const config = {
      ...nativeConfig,
      agent: { engine: "codex", acp: {} },
      session: {
        new_session_provider_id: "__acp__",
        new_session_model: "saved-model"
      }
    } as unknown as AppConfig;

    expect(buildNewSessionModelChoices(config, connectedCodexStatus).map((choice) => choice.model))
      .toEqual(["runtime-model"]);
  });

  it("retains configured ACP defaults before the first runtime handshake", () => {
    const config = {
      ...nativeConfig,
      agent: { engine: "codex", acp: {} },
      session: {
        new_session_provider_id: "__acp__",
        new_session_model: "saved-model",
        new_session_thinking_level: "xhigh"
      }
    } as unknown as AppConfig;
    const statusBeforeHandshake = { ...connectedCodexStatus, acp_runtime: null };

    expect(buildNewSessionModelChoices(
      config,
      statusBeforeHandshake
    ).map((choice) => choice.model)).toEqual(["saved-model"]);
    expect(buildNewSessionThinkingLevels(config, statusBeforeHandshake))
      .toEqual(["auto", "none", "low", "medium", "high", "xhigh", "max"]);
    expect(resolveEffectiveNewSessionPreferences(config, statusBeforeHandshake)).toEqual({
      model: { providerId: "__acp__", model: "saved-model" },
      thinkingLevel: "xhigh"
    });
  });

  it("falls back from ACP values that the connected runtime does not support", () => {
    const config = {
      ...nativeConfig,
      agent: { engine: "codex", acp: {} },
      session: {
        new_session_provider_id: "__acp__",
        new_session_model: "saved-model",
        new_session_thinking_level: "xhigh"
      }
    } as unknown as AppConfig;

    expect(resolveEffectiveNewSessionPreferences(config, connectedCodexStatus)).toEqual({
      model: null,
      thinkingLevel: "auto"
    });
  });

  it("offers only auto when ACP does not advertise thought-level support", () => {
    const config = {
      ...nativeConfig,
      agent: { engine: "codex", acp: {} }
    } as unknown as AppConfig;
    const statusWithoutThinking = {
      ...connectedCodexStatus,
      acp_runtime: {
        ...connectedCodexStatus.acp_runtime,
        config_options: [{
          id: "model",
          category: "model",
          type: "select",
          currentValue: "runtime-model",
          options: [{ value: "runtime-model", name: "Runtime model" }]
        }]
      }
    } as EngineStatusResponse;

    expect(buildNewSessionThinkingLevels(config, statusWithoutThinking)).toEqual(["auto"]);
  });

  it("applies ACP values that the connected runtime explicitly supports", () => {
    const config = {
      ...nativeConfig,
      agent: { engine: "codex", acp: {} },
      session: {
        new_session_provider_id: "__acp__",
        new_session_model: "runtime-model",
        new_session_thinking_level: "high"
      }
    } as unknown as AppConfig;

    expect(resolveEffectiveNewSessionPreferences(config, connectedCodexStatus)).toEqual({
      model: { providerId: "__acp__", model: "runtime-model" },
      thinkingLevel: "high"
    });
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

  it("resets engine-specific values and keeps provider references synchronized", () => {
    const session = {
      new_session_provider_id: "provider-a",
      new_session_model: "model-b",
      new_session_thinking_level: "high" as const,
      auto_title_enabled: false
    };

    expect(resetNewSessionEnginePreferences(session)).toEqual({
      new_session_provider_id: undefined,
      new_session_model: undefined,
      new_session_thinking_level: "auto",
      auto_title_enabled: false
    });
    expect(renameNewSessionProviderReference(session, "provider-a", "provider-b"))
      .toEqual({ ...session, new_session_provider_id: "provider-b" });
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
