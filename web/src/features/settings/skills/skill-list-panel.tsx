import { RefreshCw, Sparkles } from "lucide-react";
import type { ManagedSkill } from "../../../api/skill-contracts";
import { Button } from "../../../shared/ui/button/button";
import { ObjectListPanel } from "../object-list-panel";
import { useI18n } from "../../i18n/use-i18n";

type SkillListPanelProps = {
  skills: ManagedSkill[];
  selectedId: string;
  scanning: boolean;
  onSelect: (id: string) => void;
  onAdd: () => void;
  onScan: () => void;
};

/**
 * 渲染 Skill 扫描结果列表。
 *
 * @param props Skill 列表、选中项与操作回调
 * @returns 带扫描入口的对象列表
 */
export function SkillListPanel({ skills, selectedId, scanning, onSelect, onAdd, onScan }: SkillListPanelProps) {
  const { t } = useI18n();
  return (
    <ObjectListPanel
      title="Skills"
      items={skills.map((skill) => ({
        id: skill.id,
        name: skill.name,
        meta: `${skill.scope === "global" ? t("Global", "全局") : t("Workspace", "工作区")} / ${skill.directory_name}`,
        icon: <Sparkles size={14} />,
        marked: skill.enabled
      }))}
      selectedId={selectedId}
      searchPlaceholder={t("Search Skills", "搜索 Skills")}
      addLabel={t("Add Skill", "新增 Skill")}
      onSelect={onSelect}
      onAdd={onAdd}
      topSlot={(
        <Button className="skills-scan-button" onClick={onScan} disabled={scanning}>
          <RefreshCw size={13} className={scanning ? "is-spinning" : ""} />
          {scanning ? t("Scanning", "正在扫描") : t("Scan directories", "扫描目录")}
        </Button>
      )}
    />
  );
}
