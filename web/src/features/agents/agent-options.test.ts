import { describe, expect, it } from "vitest";
import type { AppConfig } from "../../api/contracts";
import { buildAgentChoices, DEFAULT_AGENT_ID, resolveAgentChoice } from "./agent-options";

const config = { active_provider: "provider", providers: [], gateways: {} } as unknown as AppConfig;

describe("agent options", () => {
  it("adds the virtual default agent when no profiles exist", () => {
    expect(buildAgentChoices(config)).toEqual([
      { id: DEFAULT_AGENT_ID, name: "默认 Agent" },
      { id: "general", name: "代码 Agent" },
      { id: "explore", name: "探索 Agent" },
      { id: "gateway", name: "网关 Agent" }
    ]);
  });

  it("falls back to the code agent for an invalid preference", () => {
    const choices = buildAgentChoices({ ...config, agents: [{ id: "review", name: "审查" }] } as AppConfig);
    expect(resolveAgentChoice(choices, "missing")?.id).toBe("general");
  });
});
