import { describe, expect, it } from "vitest";
import { composeSkillDocument, parseSkillDocument } from "./skill-document";

describe("skill-document", () => {
  it("parses name description and body", () => {
    const parsed = parseSkillDocument("---\nname: review\ndescription: Review code changes\n---\n\n# Steps\n\n1. read\n");
    expect(parsed).toEqual({
      name: "review",
      description: "Review code changes",
      body: "# Steps\n\n1. read",
      hasFrontmatter: true
    });
  });

  it("round-trips through compose", () => {
    const content = composeSkillDocument("drawio", "Draw diagrams", "## Usage\n\nRun /drawio");
    const parsed = parseSkillDocument(content);
    expect(parsed.name).toBe("drawio");
    expect(parsed.description).toBe("Draw diagrams");
    expect(parsed.body).toContain("## Usage");
  });
});
