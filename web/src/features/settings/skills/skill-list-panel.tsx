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
        meta: `${scopeLabel(skill.scope, t)} / ${skill.directory_name}`,
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

/**
 * 将扫描源标识转为界面文案。
 *
 * @param scope 后端 scope
 * @param t 双语函数
 * @returns 展示名
 */
function scopeLabel(scope: string, t: (en: string, zh: string) => string): string {
  const map: Record<string, [string, string]> = {
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
    project_skills: ["Project skills", "项目 skills"]
  };
  const pair = map[scope] ?? [scope, scope];
  return t(...pair);
}
