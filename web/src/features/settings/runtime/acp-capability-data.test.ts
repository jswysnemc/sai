import { describe, expect, it } from "vitest";
import type { EngineStatusResponse } from "../../../api/contracts";
import {
  groupAcpCapabilities,
  parseAcpCommands,
  resolveAcpConnectionState
} from "./acp-capability-data";

const capabilities: NonNullable<EngineStatusResponse["acp_capabilities"]> = {
  load_session: true,
  list_sessions: false,
  delete_session: false,
  resume_session: true,
  close_session: true,
  additional_directories: true,
  mcp_http: false,
  mcp_sse: false,
  prompt_image: true,
  prompt_audio: false,
  embedded_context: true,
  logout: false,
  sai_context_compaction: true,
  sai_memory: true,
  sai_goal_continuation: true,
  sai_subagents: true
};

/**
 * 构造带指定运行状态的外部内核响应。
 *
 * @param runtime ACP 运行快照
 * @returns Codex 内核状态响应
 */
function status(runtime: EngineStatusResponse["acp_runtime"]): EngineStatusResponse {
  return {
    engine: "codex",
    label: "Codex",
    external: true,
    unavailable_features: [],
    acp_capabilities: capabilities,
    acp_runtime: runtime
  };
}

describe("ACP capability data", () => {
  it("separates Codex native equivalents from Sai integrations", () => {
    const groups = groupAcpCapabilities("codex", capabilities, {
      context_compaction: "codex",
      subagents: "codex"
    });

    expect(groups.standard.map((item) => item.id)).toContain("load_session");
    expect(groups.sai.map((item) => item.id)).toEqual([
      "sai_memory",
      "sai_goal_continuation"
    ]);
    expect(groups.codexNative.map((item) => item.id)).toEqual([
      "sai_context_compaction",
      "sai_subagents"
    ]);
    expect(groups.unsupported.map((item) => item.id)).toContain("prompt_audio");
  });

  it("reports loading, disconnected, connected, partial, and error states", () => {
    expect(resolveAcpConnectionState("codex", undefined, true, null)).toBe("loading");
    expect(resolveAcpConnectionState("codex", status(null), false, null)).toBe("disconnected");
    expect(resolveAcpConnectionState("codex", status({
      connected: true,
      agent_name: "Codex",
      agent_version: "1.1.7",
      capabilities,
      auth_methods: [],
      config_options: [],
      modes: null,
      available_commands: [],
      native_equivalents: {
        context_compaction: "codex",
        subagents: "codex"
      }
    }), false, null)).toBe("connected");
    expect(resolveAcpConnectionState("codex", {
      ...status(null),
      acp_capabilities: { ...capabilities, sai_memory: false },
      acp_runtime: {
        connected: true,
        agent_name: "Codex",
        agent_version: "1.1.7",
        capabilities: { ...capabilities, sai_memory: false },
        auth_methods: [],
        config_options: [],
        modes: null,
        available_commands: [],
        native_equivalents: {
          context_compaction: "codex",
          subagents: "codex"
        }
      }
    }, false, null)).toBe("partial");
    expect(resolveAcpConnectionState("codex", undefined, false, new Error("offline"))).toBe("error");
  });

  it("keeps the latest runtime snapshot but reports a closed process as disconnected", () => {
    expect(resolveAcpConnectionState("codex", status({
      connected: false,
      agent_name: "Codex",
      agent_version: "1.1.7",
      capabilities,
      auth_methods: [],
      config_options: [],
      modes: null,
      available_commands: [],
      native_equivalents: {}
    }), false, null)).toBe("disconnected");
  });

  it("treats a cached response for another engine as disconnected", () => {
    expect(resolveAcpConnectionState("claude_code", status(null), false, null)).toBe("disconnected");
  });

  it("normalizes valid slash commands and ignores malformed values", () => {
    expect(parseAcpCommands([
      { name: "compact", description: "Compact context" },
      { name: "/review" },
      { description: "missing name" },
      null
    ])).toEqual([
      { name: "/compact", description: "Compact context" },
      { name: "/review", description: "" }
    ]);
  });
});
