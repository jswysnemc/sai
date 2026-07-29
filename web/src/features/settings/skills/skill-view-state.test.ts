import { describe, expect, it } from "vitest";
import type { ManagedSkill } from "../../../api/skill-contracts";
import { normalizeSkillLibraryPage, type SkillLibraryPage } from "./skill-view-state";

const installedSkill: ManagedSkill = {
  id: "global/code-review",
  name: "code-review",
  description: "Review source changes",
  scope: "global",
  directory_name: "code-review",
  path: "/skills/code-review/SKILL.md",
  enabled: true
};

describe("normalizeSkillLibraryPage", () => {
  it("保留网格、新建和仍存在的详情状态", () => {
    const pages: SkillLibraryPage[] = [
      { kind: "grid" },
      { kind: "create" },
      { kind: "detail", skillId: installedSkill.id }
    ];

    expect(pages.map((page) => normalizeSkillLibraryPage(page, [installedSkill]))).toEqual(pages);
  });

  it("详情目标从扫描结果消失后返回网格", () => {
    expect(normalizeSkillLibraryPage({ kind: "detail", skillId: installedSkill.id }, [])).toEqual({ kind: "grid" });
  });
});
