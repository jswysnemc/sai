import type { ManagedSkill } from "../../../api/skill-contracts";
import type { SkillStatusFilter } from "./skill-list-filter";

/** 技能库内部页面状态。 */
export type SkillLibraryPage =
  | { kind: "grid" }
  | { kind: "detail"; skillId: string }
  | { kind: "create" };

/** 技能库筛选条件。 */
export type SkillLibraryFilters = {
  query: string;
  status: SkillStatusFilter;
  scope: string;
};

/** 技能库默认筛选条件。 */
export const INITIAL_SKILL_LIBRARY_FILTERS: SkillLibraryFilters = {
  query: "",
  status: "all",
  scope: "all"
};

/**
 * 在扫描结果变化后校正技能库页面状态。
 *
 * @param page 当前技能库页面
 * @param skills 最新扫描得到的 Skill
 * @returns 可继续展示的页面；详情目标消失时返回网格页
 */
export function normalizeSkillLibraryPage(page: SkillLibraryPage, skills: ManagedSkill[]): SkillLibraryPage {
  if (page.kind === "detail" && !skills.some((skill) => skill.id === page.skillId)) {
    return { kind: "grid" };
  }
  return page;
}
