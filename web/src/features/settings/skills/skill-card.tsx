import { BookOpen, ChevronRight, FolderCode, Globe2 } from "lucide-react";
import type { ManagedSkill } from "../../../api/skill-contracts";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import { skillScopeLabel } from "./skill-list-filter";

type SkillCardProps = {
  skill: ManagedSkill;
  onOpen: (id: string) => void;
};

/**
 * 渲染可进入详情设置页的 Skill 卡片。
 *
 * @param props Skill 数据与打开详情回调
 * @returns 网格中的单个 Skill 卡片
 */
export function SkillCard({ skill, onOpen }: SkillCardProps) {
  const { t } = useI18n();
  const isProjectSkill = skill.scope.startsWith("project_");

  return (
    <li className="skill-grid-item">
      <Button className="skill-card" title={skill.path} onClick={() => onOpen(skill.id)}>
        <span className="skill-card-topline">
          <span className="skill-card-icon" aria-hidden="true">
            {isProjectSkill
              ? <FolderCode size={16} />
              : skill.scope === "global"
                ? <Globe2 size={16} />
                : <BookOpen size={16} />}
          </span>
          <span className="skill-card-status" data-enabled={skill.enabled}>
            <i aria-hidden="true" />
            {skill.enabled ? t("Enabled", "已启用") : t("Disabled", "已禁用")}
          </span>
        </span>
        <span className="skill-card-copy">
          <strong>{skill.name}</strong>
          <small>{skill.description || t("No description", "暂无说明")}</small>
        </span>
        <span className="skill-card-footer">
          <span>
            <span>{skillScopeLabel(skill.scope, t)}</span>
            <code>{skill.directory_name}</code>
          </span>
          <ChevronRight size={15} aria-hidden="true" />
        </span>
      </Button>
    </li>
  );
}
