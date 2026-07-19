import { describe, expect, it } from "vitest";
import {
  collectSkillMentionNames,
  expandSkillMentions,
  findSkillMentionTrigger,
  formatSkillMention,
  parseSkillMentions
} from "./skill-mention-token";

describe("skill mention token", () => {
  it("parses slash skill tokens at text boundaries", () => {
    expect(parseSkillMentions("用 /drawio 画图，再看看 /research")).toEqual([
      { type: "text", value: "用 " },
      { type: "skill", name: "drawio", value: "/drawio" },
      { type: "text", value: " 画图，再看看 " },
      { type: "skill", name: "research", value: "/research" }
    ]);
  });

  it("detects a slash skill trigger under the caret", () => {
    expect(findSkillMentionTrigger("/", 1)).toEqual({ start: 0, end: 1, query: "" });
    expect(findSkillMentionTrigger("请用 /dra", 7)).toEqual({ start: 3, end: 7, query: "dra" });
    expect(findSkillMentionTrigger("http://x", 8)).toBeNull();
    expect(findSkillMentionTrigger("a/b", 3)).toBeNull();
  });

  it("formats and expands skill documents for model input", () => {
    expect(formatSkillMention("drawio")).toBe("/drawio");
    expect(collectSkillMentionNames("先 /drawio 再 /drawio")).toEqual(["drawio"]);
    expect(
      expandSkillMentions("先 /drawio 后继续", {
        drawio: "<loaded-skill name=\"drawio\">body</loaded-skill>"
      })
    ).toBe("先 <skill-reference name=\"drawio\">\n<loaded-skill name=\"drawio\">body</loaded-skill>\n</skill-reference> 后继续");
  });
});
