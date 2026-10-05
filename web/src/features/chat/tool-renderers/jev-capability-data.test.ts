import { describe, expect, it } from "vitest";
import { jevCapabilityStatusLabel, jevSelectionSummary, parseJevCapability, parseJevExposureBlock } from "./jev-capability-data";

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
  it("保留工具名、说明首句、完整说明、顶层参数和 skill 状态，不带 skill 全文", () => {
    expect(parseJevCapability(exposed)).toEqual({
      tools: [{
        kind: "tool",
        name: "web_search",
        detail: "Search the web.",
        description: "Search the web. Returns ranked pages.",
        parameters: [{ name: "query", type: "string", required: false, description: "" }]
      }],
      skills: [{ kind: "skill", name: "drawio", detail: "loaded" }],
      contexts: []
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
    expect(parseJevCapability(output)).toEqual({ tools: [], skills: [], contexts: [] });
    expect(jevCapabilityStatusLabel(parseJevCapability(output)!, "zh-CN")).toBe("未匹配");
  });

  it("工具错误和其它工具结果不按能力申请解析", () => {
    expect(parseJevCapability("tool error: jev unavailable")).toBeNull();
    expect(parseJevCapability(JSON.stringify({ ok: true, tools: [] }))).toBeNull();
  });

  it("从预选注入块取出名单，并忽略标签外的说明", () => {
    const content = `<context-state>{"keep":true}</context-state>\n<jev-exposed-capabilities>\nBefore this request\n${exposed}\n</jev-exposed-capabilities>`;
    expect(parseJevExposureBlock(content)?.tools.map((item) => item.name)).toEqual(["web_search"]);
    expect(parseJevExposureBlock(content)?.skills[0]?.detail).toBe("loaded");
    expect(parseJevExposureBlock("no jev here")).toBeNull();
  });

  it("按数量生成折叠行计数", () => {
    const exposure = parseJevCapability(exposed)!;
    expect(jevCapabilityStatusLabel(exposure, "zh-CN")).toBe("1 个工具 · 1 个 Skill");
    expect(jevCapabilityStatusLabel(exposure, "en-US")).toBe("1 tool · 1 skill");
  });

  it("发送前摘要写成一句话，中英文连接词自然", () => {
    const exposure = parseJevCapability(exposed)!;
    expect(jevSelectionSummary(exposure, "zh-CN")).toBe("选用了 1 个工具和 1 个 Skill");
    expect(jevSelectionSummary(exposure, "en-US")).toBe("Picked 1 tool and 1 skill");
    const withMemory = {
      ...exposure,
      tools: [...exposure.tools, { kind: "tool" as const, name: "browser", detail: "" }],
      contexts: [{ kind: "memory" as const, id: "m", source: "memory", description: "", preview: "" }]
    };
    expect(jevSelectionSummary(withMemory, "zh-CN")).toBe("选用了 2 个工具、1 个 Skill 和记忆");
    expect(jevSelectionSummary(withMemory, "en-US")).toBe("Picked 2 tools, 1 skill and memory");
    expect(jevSelectionSummary({ ...exposure, tools: [] }, "zh-CN")).toBe("选用了 1 个 Skill");
  });

  it("预选 detail 中的片段与记忆按种类解析并计入折叠行", () => {
    const detail = JSON.stringify({
      ok: true,
      router: "jev",
      tools: [],
      skills: [],
      contexts: [
        { id: "prompt_1", kind: "prompt", source: "system", description: "编写迁移时使用", preview: "迁移必须可回退" },
        { id: "memory_context", kind: "memory", source: "memory", description: "Memory index", preview: "- [偏好](a.md)" },
        { kind: "unknown" }
      ]
    });
    const exposure = parseJevCapability(detail)!;
    expect(exposure.contexts.map((item) => item.kind)).toEqual(["prompt", "memory"]);
    expect(exposure.contexts[0]?.preview).toBe("迁移必须可回退");
    expect(jevCapabilityStatusLabel(exposure, "zh-CN")).toBe("1 个片段 · 记忆");
  });

  it("历史注入只有片段块时也能识别，并从 memory 块取索引", () => {
    const content = [
      "<memory>\n说明。\n\n项目记忆：\n- [偏好](a.md) — 用 pnpm\n</memory>",
      "<jev-selected-context>\nThe Jev router selected...",
      "<jev-context id=\"prompt_1\" kind=\"prompt\" source=\"instructions\" description=\"迁移 &quot;规范&quot;\">\n迁移必须可回退\n</jev-context>",
      "<jev-context id=\"memory_context\">\n契约\n</jev-context>",
      "</jev-selected-context>"
    ].join("\n");
    const exposure = parseJevExposureBlock(content)!;
    expect(exposure.tools).toEqual([]);
    expect(exposure.contexts).toEqual([
      { kind: "prompt", id: "prompt_1", source: "instructions", description: "迁移 \"规范\"", preview: "迁移必须可回退" },
      { kind: "memory", id: "memory_context", source: "", description: "", preview: "项目记忆：\n- [偏好](a.md) — 用 pnpm" }
    ]);
  });
});
