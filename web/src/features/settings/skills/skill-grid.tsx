import { Plus, RefreshCw, Search } from "lucide-react";
import { useMemo } from "react";
import type { ManagedSkill } from "../../../api/skill-contracts";
import { Button } from "../../../shared/ui/button/button";
import { Select } from "../../../shared/ui/select/select";
import { useI18n } from "../../i18n/use-i18n";
import { SkillCard } from "./skill-card";
import { filterManagedSkills, skillScopeLabel, type SkillStatusFilter } from "./skill-list-filter";
import type { SkillLibraryFilters } from "./skill-view-state";
import "./skill-grid.css";

type SkillGridProps = {
  skills: ManagedSkill[];
  filters: SkillLibraryFilters;
  scanning: boolean;
  error: string | null;
  onFiltersChange: (filters: SkillLibraryFilters) => void;
  onOpen: (id: string) => void;
  onAdd: () => void;
  onScan: () => void;
};

/**
 * 渲染带搜索和筛选功能的响应式 Skill 网格。
 *
 * @param props Skill 数据、筛选条件、请求状态与操作回调
 * @returns 技能库网格页
 */
export function SkillGrid({ skills, filters, scanning, error, onFiltersChange, onOpen, onAdd, onScan }: SkillGridProps) {
  const { t } = useI18n();
  const scopes = useMemo(() => [...new Set(skills.map((skill) => skill.scope))].sort(), [skills]);
  const visibleSkills = useMemo(
    () => filterManagedSkills(skills, filters.query, filters.status, filters.scope),
    [filters.query, filters.scope, filters.status, skills]
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
  const resultCount = visibleSkills.length === skills.length
    ? String(skills.length)
    : `${visibleSkills.length}/${skills.length}`;

  return (
    <section className="skill-library-view" aria-label={t("Skills library", "Skills 技能库")} aria-busy={scanning}>
      <header className="skill-library-header">
        <div>
          <h2>{t("Skills", "Skills")}</h2>
          <p>{t("Open a Skill to inspect its instructions and settings.", "打开 Skill 后查看并设置其指令与状态。")}</p>
        </div>
        <div className="skill-library-actions">
          <Button onClick={onScan} disabled={scanning}>
            <RefreshCw size={14} className={scanning ? "is-spinning" : ""} />
            {t("Scan", "扫描")}
          </Button>
          <Button variant="primary" onClick={onAdd}>
            <Plus size={14} />
            {t("Add Skill", "新增 Skill")}
          </Button>
        </div>
      </header>

      <div className="skill-library-controls">
        <label className="skill-library-search">
          <Search size={14} aria-hidden="true" />
          <input
            type="search"
            value={filters.query}
            onChange={(event) => onFiltersChange({ ...filters, query: event.target.value })}
            placeholder={t("Search name, description, or source", "搜索名称、说明或来源")}
            aria-label={t("Search Skills", "搜索 Skills")}
            spellCheck={false}
          />
        </label>
        <div className="skill-library-filters">
          <Select
            value={filters.status}
            options={statusOptions}
            ariaLabel={t("Filter Skill status", "筛选 Skill 状态")}
            menuMinimumWidth={140}
            onChange={(status) => onFiltersChange({ ...filters, status })}
          />
          <Select
            value={filters.scope}
            options={scopeOptions}
            ariaLabel={t("Filter Skill source", "筛选 Skill 来源")}
            menuMinimumWidth={168}
            onChange={(scope) => onFiltersChange({ ...filters, scope })}
          />
        </div>
        <span className="skill-library-count">{t(`${resultCount} results`, `${resultCount} 项`)}</span>
      </div>

      {error && <div className="settings-inline-error">{error}</div>}
      {scanning && (
        <div className="skill-library-scan-status" role="status">
          <RefreshCw size={13} className="is-spinning" />
          {t("Scanning Skill directories", "正在扫描 Skill 目录")}
        </div>
      )}

      {visibleSkills.length > 0 ? (
        <ul className="skill-grid" aria-label={t("Available Skills", "可用 Skills")}>
          {visibleSkills.map((skill) => <SkillCard key={skill.id} skill={skill} onOpen={onOpen} />)}
        </ul>
      ) : (
        <div className="skill-grid-empty">
          <Search size={19} aria-hidden="true" />
          <strong>{skills.length === 0 ? t("No Skills found", "尚未发现 Skill") : t("No matching Skills", "没有匹配的 Skill")}</strong>
          <span>{skills.length === 0
            ? t("Scan configured directories or add a global Skill.", "扫描已配置目录，或新增一个全局 Skill。")
            : t("Adjust the search or filters to see more results.", "调整搜索词或筛选条件后重试。")}</span>
        </div>
      )}
    </section>
  );
}
