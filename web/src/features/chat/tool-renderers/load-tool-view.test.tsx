import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { toolResultSummary } from "./tool-result-summary";
import { parseLoadResult, splitFrontmatter } from "./load-tool-data";
import { LoadToolView } from "./load-tool-view";

const skillOutput = JSON.stringify({
  ok: true,
  skills: [
    {
      name: "drawio",
      status: "loaded",
      content: "---\nname: drawio\ndescription: \"Draw diagrams with draw.io\"\n---\n\n## Steps\n\nExport as **SVG**."
    },
    { name: "pdf", status: "already_loaded" }
  ],
  already_loaded: false,
  instruction: "..."
});

const toolOutput = JSON.stringify({
  ok: true,
  tools: [{
    name: "web_search",
    status: "loaded",
    definition: {
      type: "function",
      function: {
        name: "web_search",
        description: "Search the web. Returns ranked pages.",
        parameters: {
          type: "object",
          properties: {
            query: { type: "string", description: "Search terms" },
            domains: { type: "array", items: { type: "string" } },
            mode: { type: "string", enum: ["fast", "deep"] }
          },
          required: ["query"]
        }
      }
    }
  }]
});

describe("load 结果解析", () => {
  it("拆出 Skill 的 frontmatter 说明与正文", () => {
    const result = parseLoadResult(skillOutput);
    expect(result?.kind).toBe("skill");
    if (result?.kind !== "skill") return;
    expect(result.items[0]).toEqual({
      name: "drawio",
      description: "Draw diagrams with draw.io",
      status: "loaded",
      body: "## Steps\n\nExport as **SVG**."
    });
    expect(result.items[1]).toMatchObject({ name: "pdf", status: "already_loaded", body: "" });
  });

  it("读取工具参数名、类型与必填", () => {
    const result = parseLoadResult(toolOutput);
    expect(result?.kind).toBe("tool");
    if (result?.kind !== "tool") return;
    expect(result.items[0]?.parameters).toEqual([
      { name: "query", type: "string", required: true, description: "Search terms" },
      { name: "domains", type: "string[]", required: false, description: "" },
      { name: "mode", type: "enum", required: false, description: "" }
    ]);
  });

  it("非 load 成功结果返回空，没有 frontmatter 时正文原样保留", () => {
    expect(parseLoadResult("tool error: unknown skill")).toBeNull();
    expect(parseLoadResult(JSON.stringify({ ok: true }))).toBeNull();
    expect(splitFrontmatter("# Plain").body).toBe("# Plain");
  });

  it("折叠行摘要给出加载数量", () => {
    expect(toolResultSummary("load", skillOutput, "zh-CN")?.label).toBe("2 份 Skill 文档");
    expect(toolResultSummary("load", toolOutput, "en-US")?.label).toBe("1 tool schema");
  });
});

describe("LoadToolView", () => {
  it("Skill 显示名称与说明，正文默认折叠但可展开", () => {
    const result = parseLoadResult(skillOutput)!;
    const html = renderToStaticMarkup(<LoadToolView result={result} />);
    expect(html).toContain("drawio");
    expect(html).toContain("Draw diagrams with draw.io");
    expect(html).toContain("icon-label");
    expect(html).toContain("disclosure-list");
    expect(html).toContain("此前已加载");
    expect(html).toContain('aria-expanded="false"');
    // 正文已渲染进折叠容器，展开时无需再请求
    expect(html).toContain("Steps");
    expect(html).not.toContain("name: drawio");
  });

  it("工具显示首句说明与参数表，必填参数带标记", () => {
    const html = renderToStaticMarkup(<LoadToolView result={parseLoadResult(toolOutput)!} />);
    expect(html).toContain("Search the web.");
    expect(html).not.toContain("Returns ranked pages.");
    expect(html).toContain("<code>query</code>");
    expect(html).toContain("load-tool-required");
    expect(html).toContain("string[]");
  });
});
