import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { parseJevCapability } from "./jev-capability-data";
import { JevCapabilityView } from "./jev-capability-view";

describe("JevCapabilityView", () => {
  it("列出暴露的工具和 Skill，不渲染 Schema 与全文", () => {
    const output = JSON.stringify({
      ok: true,
      router: "jev",
      tools: [{
        name: "web_search",
        definition: {
          type: "function",
          function: {
            name: "web_search",
            description: "Search the web.",
            parameters: { secret_schema_field: true }
          }
        }
      }],
      skills: [{ name: "drawio", status: "loaded", content: "SKILL_DOCUMENT_BODY" }]
    });
    const html = renderToStaticMarkup(
      <JevCapabilityView
        argumentsText={JSON.stringify({ need: "search the web and draw a diagram" })}
        exposure={parseJevCapability(output)!}
      />
    );
    expect(html).toContain("search the web and draw a diagram");
    expect(html).toContain("web_search");
    expect(html).toContain("Search the web.");
    expect(html).toContain("drawio");
    expect(html).toContain("新暴露");
    expect(html).not.toContain("secret_schema_field");
    expect(html).not.toContain("SKILL_DOCUMENT_BODY");
  });

  it("没有匹配时说明未再暴露资源", () => {
    const html = renderToStaticMarkup(
      <JevCapabilityView
        argumentsText={JSON.stringify({ need: "invent a language" })}
        exposure={{ tools: [], skills: [], contexts: [] }}
      />
    );
    expect(html).toContain("Jev 没有再暴露工具或 Skill。");
  });

  it("按工具、片段、记忆分区，片段与记忆可展开预览", () => {
    const html = renderToStaticMarkup(
      <JevCapabilityView
        argumentsText="{}"
        exposure={{
          tools: [{ kind: "tool", name: "web_search", detail: "Search the web." }],
          skills: [],
          contexts: [
            { kind: "prompt", id: "prompt_1", source: "instructions", description: "编写迁移时使用", preview: "迁移必须可回退" },
            { kind: "memory", id: "memory_context", source: "memory", description: "", preview: "项目记忆：\n- [偏好](a.md)\n- [规范](b.md)" }
          ]
        }}
      />
    );
    expect(html).toContain("提示词片段");
    expect(html).toContain("编写迁移时使用");
    expect(html).toContain("指令文件");
    expect(html).toContain("记忆上下文");
    expect(html).toContain("2 条记忆索引");
    expect(html).toContain("迁移必须可回退");
    expect(html).toContain('aria-expanded="false"');
  });
});
