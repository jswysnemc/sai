import type { ManagedSkill } from "../../../api/skill-contracts";

export type SkillStatusFilter = "all" | "enabled" | "disabled";

/**
 * 按搜索词、启用状态和扫描来源过滤 Skill。
 *
 * @param skills 全部可管理 Skill
 * @param query 名称、说明、目录或来源搜索词
 * @param status 启用状态筛选
 * @param scope 扫描来源筛选，all 表示全部
 * @returns 保持原顺序的匹配 Skill
 */
export function filterManagedSkills(
  skills: ManagedSkill[],
  query: string,
  status: SkillStatusFilter,
  scope: string
): ManagedSkill[] {
  const keyword = query.trim().toLocaleLowerCase();
  return skills.filter((skill) => {
    const matchesQuery = !keyword || [skill.name, skill.description, skill.directory_name, skill.scope]
      .some((value) => value.toLocaleLowerCase().includes(keyword));
    const matchesStatus = status === "all"
      || (status === "enabled" ? skill.enabled : !skill.enabled);
    const matchesScope = scope === "all" || skill.scope === scope;
    return matchesQuery && matchesStatus && matchesScope;
  });
}

/**
 * 将扫描源标识转换为紧凑界面文案。
 *
 * @param scope 后端扫描源标识
 * @param t 双语翻译函数
 * @returns 扫描来源展示名
 */
export function skillScopeLabel(scope: string, t: (en: string, zh: string) => string): string {
  const labels: Record<string, [string, string]> = {
    global: ["Global", "全局"],
    persona: ["Persona", "人格"],
    claude: ["Claude", "Claude"],
    codex: ["Codex", "Codex"],
    agents: ["Agents", "Agents"],
    agent: ["Agent", "Agent"],
    opencode: ["OpenCode", "OpenCode"],
    opencode_home: ["OpenCode", "OpenCode"],
    project_claude: ["Project Claude", "项目 Claude"],
    project_codex: ["Project Codex", "项目 Codex"],
    project_agents: ["Project Agents", "项目 Agents"],
    project_agent: ["Project Agent", "项目 Agent"],
    project_opencode: ["Project OpenCode", "项目 OpenCode"],
    project_skills: ["Project skills", "项目 Skills"]
  };
  return t(...(labels[scope] ?? [scope, scope]));
}
