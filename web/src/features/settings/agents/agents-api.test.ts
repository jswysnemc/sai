import { describe, expect, it } from "vitest";
import { mergeAgentOptions } from "./agents-api";

describe("mergeAgentOptions", () => {
  it("keeps local options and appends unique MCP tools", () => {
    const result = mergeAgentOptions(
      {
        tools: [{ name: "read_file", group: "base" }],
        skills: [{ name: "review", description: "Review code" }]
      },
      {
        tools: [
          { name: "read_file", group: "base" },
          { name: "mcp_docs_search", group: "mcp" }
        ],
        skills: []
      }
    );

    expect(result.tools.map((tool) => tool.name)).toEqual(["read_file", "mcp_docs_search"]);
    expect(result.skills.map((skill) => skill.name)).toEqual(["review"]);
  });
});
