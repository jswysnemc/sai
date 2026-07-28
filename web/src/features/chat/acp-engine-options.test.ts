import { describe, expect, it } from "vitest";
import type { AppConfig, EngineStatusResponse } from "../../api/contracts";
import { acpThinkingLevels, buildAcpModelChoices, currentAcpModel } from "./acp-engine-options";

const config = {
  agent: { engine: "claude_code", acp: { model: "configured-model" } }
} as unknown as AppConfig;

const status = {
  engine: "claude_code",
  label: "Claude Code",
  external: true,
  unavailable_features: [],
  acp_runtime: {
    connected: true,
    agent_name: "Claude Code",
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
        currentValue: "sonnet",
        options: [{ value: "sonnet", name: "Sonnet" }, { value: "opus", name: "Opus" }]
      },
      {
        id: "thought",
        category: "thought_level",
        type: "select",
        currentValue: "medium",
        options: [{ value: "low", name: "Low" }, { value: "medium", name: "Medium" }]
      }
    ]
  }
} as EngineStatusResponse;

describe("ACP engine options", () => {
  it("uses models and current value from the agent runtime state", () => {
    expect(buildAcpModelChoices(status, config).map((item) => item.model)).toEqual(["sonnet", "opus"]);
    expect(currentAcpModel(status)).toBe("sonnet");
  });

  it("limits thinking choices to values advertised by the agent", () => {
    expect(acpThinkingLevels(status)).toEqual(["low", "medium"]);
  });

  it("uses the configured model before the first session handshake", () => {
    expect(buildAcpModelChoices({ ...status, acp_runtime: null }, config)[0]?.model).toBe("configured-model");
  });
});
