import { BookOpen, FolderCode, Globe2, Plus, RefreshCw, Search } from "lucide-react";
import { useMemo, useState } from "react";
import type { ManagedSkill } from "../../../api/skill-contracts";
import { Button } from "../../../shared/ui/button/button";
import { Select } from "../../../shared/ui/select/select";
import { useI18n } from "../../i18n/use-i18n";
import { filterManagedSkills, skillScopeLabel, type SkillStatusFilter } from "./skill-list-filter";

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
  const [query, setQuery] = useState("");
  const [status, setStatus] = useState<SkillStatusFilter>("all");
  const [scope, setScope] = useState("all");
  const scopes = useMemo(() => [...new Set(skills.map((skill) => skill.scope))].sort(), [skills]);
  const visibleSkills = useMemo(
    () => filterManagedSkills(skills, query, status, scope),
    [query, scope, skills, status]
  );
  const statusOptions = [
    { value: "all", label: t("All statuses", "全部状态") },
    { value: "enabled", label: t("Enabled", "已启用") },
    { value: "disabled", label: t("Disabled", "已禁用") }
  ] satisfies Array<{ value: SkillStatusFilter; label: string }>;
  const scopeOptions = [
    { value: "all", label: t("All sources", "全部来源") },
    ...scopes.map((value) => ({ value, label: skillScopeLabel(value, t) }))
  ];

  return (
    <aside className="skill-list-panel" aria-label={t("Skills library", "Skills 技能库")} aria-busy={scanning}>
      <header className="skill-list-header">
        <div>
          <h2>{t("Skills", "Skills")}</h2>
          <span>{visibleSkills.length === skills.length ? skills.length : `${visibleSkills.length}/${skills.length}`}</span>
        </div>
        <div className="skill-list-actions">
          <Button className="skill-list-icon-button" onClick={onScan} disabled={scanning} aria-label={t("Scan directories", "扫描目录")} title={t("Scan directories", "扫描目录")}>
            <RefreshCw size={14} className={scanning ? "is-spinning" : ""} />
          </Button>
          <Button className="skill-list-icon-button" onClick={onAdd} aria-label={t("Add Skill", "新增 Skill")} title={t("Add Skill", "新增 Skill")}>
            <Plus size={15} />
          </Button>
        </div>
      </header>

      <label className="skill-list-search">
        <Search size={14} aria-hidden="true" />
        <input
          type="search"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={t("Search name, description, or source", "搜索名称、说明或来源")}
          aria-label={t("Search Skills", "搜索 Skills")}
          spellCheck={false}
        />
      </label>

      <div className="skill-list-filters">
        <Select
          value={status}
          options={statusOptions}
          ariaLabel={t("Filter Skill status", "筛选 Skill 状态")}
          menuMinimumWidth={140}
          onChange={setStatus}
        />
        <Select
          value={scope}
          options={scopeOptions}
          ariaLabel={t("Filter Skill source", "筛选 Skill 来源")}
          menuMinimumWidth={168}
          onChange={setScope}
        />
      </div>

      <div className="skill-list-scroll" role="listbox" aria-label={t("Available Skills", "可用 Skills")}>
        {visibleSkills.map((skill) => (
          <Button
            className={skill.id === selectedId ? "skill-list-item active" : "skill-list-item"}
            role="option"
            aria-selected={skill.id === selectedId}
            key={skill.id}
            title={skill.path}
            onClick={() => onSelect(skill.id)}
          >
            <span className="skill-list-item-icon">
              {skill.scope.startsWith("project_") ? <FolderCode size={14} /> : skill.scope === "global" ? <Globe2 size={14} /> : <BookOpen size={14} />}
            </span>
            <span className="skill-list-item-copy">
              <strong>{skill.name}</strong>
              <small>{skill.description || t("No description", "暂无说明")}</small>
              <span>{skillScopeLabel(skill.scope, t)} / {skill.directory_name}</span>
            </span>
            <span className="skill-list-status" data-enabled={skill.enabled}>
              <i aria-hidden="true" />
              {skill.enabled ? t("On", "启用") : t("Off", "关闭")}
            </span>
          </Button>
        ))}
        {visibleSkills.length === 0 && (
          <div className="skill-list-empty">
            <Search size={18} aria-hidden="true" />
            <span>{t("No Skills match these filters", "没有符合当前筛选条件的 Skill")}</span>
          </div>
        )}
      </div>
      {scanning && (
        <div className="skill-list-scan-status" role="status">
          <RefreshCw size={13} className={scanning ? "is-spinning" : ""} />
          {t("Scanning Skill directories", "正在扫描 Skill 目录")}
        </div>
      )}
    </aside>
  );
}
