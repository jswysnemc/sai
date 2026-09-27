import { describe, expect, it } from "vitest";
import { jevCapabilityStatusLabel, parseJevCapability } from "./jev-capability-data";

const exposed = JSON.stringify({
  ok: true,
  router: "jev",
  tools: [{
    name: "web_search",
    definition: {
      type: "function",
      function: {
        name: "web_search",
        description: "Search the web. Returns ranked pages.",
        parameters: { type: "object", properties: { query: { type: "string" } } }
      }
    }
  }],
  skills: [{
    name: "drawio",
    status: "loaded",
    content: "# drawio\nfull document that must stay out of the card"
  }],
  instruction: "Jev exposed the resources below."
});

describe("parseJevCapability", () => {
  it("只保留工具名、说明首句和 skill 状态", () => {
    expect(parseJevCapability(exposed)).toEqual({
      tools: [{ kind: "tool", name: "web_search", detail: "Search the web." }],
      skills: [{ kind: "skill", name: "drawio", detail: "loaded" }]
    });
  });

  it("未匹配时返回空名单", () => {
    const output = JSON.stringify({
      ok: true,
      router: "jev",
      tools: [],
      skills: [],
      instruction: "none"
    });
    expect(parseJevCapability(output)).toEqual({ tools: [], skills: [] });
    expect(jevCapabilityStatusLabel(parseJevCapability(output)!, "zh-CN")).toBe("未匹配");
  });

  it("工具错误和其它工具结果不按能力申请解析", () => {
    expect(parseJevCapability("tool error: jev unavailable")).toBeNull();
    expect(parseJevCapability(JSON.stringify({ ok: true, tools: [] }))).toBeNull();
  });

  it("按数量生成折叠行计数", () => {
    const exposure = parseJevCapability(exposed)!;
    expect(jevCapabilityStatusLabel(exposure, "zh-CN")).toBe("1 个工具 · 1 个 Skill");
    expect(jevCapabilityStatusLabel(exposure, "en-US")).toBe("1 tool · 1 skill");
  });
});
