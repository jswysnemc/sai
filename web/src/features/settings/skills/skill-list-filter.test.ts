import { describe, expect, it } from "vitest";
import type { ManagedSkill } from "../../../api/skill-contracts";
import { filterManagedSkills } from "./skill-list-filter";

const skills: ManagedSkill[] = [
  {
    id: "global/review",
    name: "code-review",
    description: "Review source changes",
    scope: "global",
    directory_name: "review",
    path: "/skills/review/SKILL.md",
    enabled: true
  },
  {
    id: "project/research",
    name: "research",
    description: "Collect primary sources",
    scope: "project_skills",
    directory_name: "source-research",
    path: "/project/.skills/research/SKILL.md",
    enabled: false
  }
];

describe("filterManagedSkills", () => {
  it("按名称、说明和目录搜索", () => {
    expect(filterManagedSkills(skills, "review", "all", "all")).toEqual([skills[0]]);
    expect(filterManagedSkills(skills, "primary", "all", "all")).toEqual([skills[1]]);
    expect(filterManagedSkills(skills, "source-research", "all", "all")).toEqual([skills[1]]);
  });

  it("组合启用状态与来源筛选", () => {
    expect(filterManagedSkills(skills, "", "enabled", "global")).toEqual([skills[0]]);
    expect(filterManagedSkills(skills, "", "disabled", "project_skills")).toEqual([skills[1]]);
    expect(filterManagedSkills(skills, "", "enabled", "project_skills")).toEqual([]);
  });
});
