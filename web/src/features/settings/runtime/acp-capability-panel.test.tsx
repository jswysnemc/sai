import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { EngineStatusResponse } from "../../../api/contracts";
import { AcpCapabilityPanel } from "./acp-capability-panel";

const capabilities: NonNullable<EngineStatusResponse["acp_capabilities"]> = {
  load_session: true,
  list_sessions: true,
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

const status: EngineStatusResponse = {
  engine: "codex",
  label: "Codex",
  external: true,
  unavailable_features: [],
  acp_capabilities: capabilities,
  acp_runtime: {
    connected: true,
    agent_name: "Codex",
    agent_version: "1.1.7",
    capabilities,
    auth_methods: [],
    config_options: [],
    modes: null,
    available_commands: [
      { name: "compact", description: "Compact context" },
      { name: "review", description: "Review changes" }
    ],
    native_equivalents: {
      context_compaction: "codex",
      subagents: "codex"
    }
  }
};

describe("AcpCapabilityPanel", () => {
  it("renders connected runtime identity, grouped capabilities, and slash commands", () => {
    const html = renderToStaticMarkup(
      <AcpCapabilityPanel engine="codex" status={status} loading={false} error={null} />
    );

    expect(html).toContain("已连接");
    expect(html).toContain("Codex");
    expect(html).toContain('data-agent-engine-brand="codex"');
    expect(html).toContain("1.1.7");
    expect(html).toContain("标准 ACP 能力");
    expect(html).toContain("Sai 集成能力");
    expect(html).toContain("Codex 原生等价能力");
    expect(html).toContain("未支持能力");
    expect(html).toContain("/compact");
    expect(html).toContain("音频输入");
  });

  it("renders a compact disconnected state before the first handshake", () => {
    const html = renderToStaticMarkup(
      <AcpCapabilityPanel engine="codex" status={{ ...status, acp_runtime: null }} loading={false} error={null} />
    );

    expect(html).toContain("尚未连接");
    expect(html).toContain("开始一次对话");
    expect(html).not.toContain("Codex 原生等价能力");
  });

  it("retains the latest capability snapshot after the process disconnects", () => {
    const html = renderToStaticMarkup(
      <AcpCapabilityPanel
        engine="codex"
        status={{ ...status, acp_runtime: { ...status.acp_runtime!, connected: false } }}
        loading={false}
        error={null}
      />
    );

    expect(html).toContain("尚未连接");
    expect(html).toContain("最近一次握手快照");
    expect(html).toContain("Codex 原生等价能力");
  });
});
