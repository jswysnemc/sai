/** 设置页扫描得到的可管理 Skill。 */
export type ManagedSkill = {
  id: string;
  name: string;
  description: string;
  scope: "global" | "persona";
  directory_name: string;
  path: string;
  enabled: boolean;
};

/** Skill 元数据与完整 SKILL.md 文档。 */
export type ManagedSkillDocument = {
  skill: ManagedSkill;
  content: string;
};
